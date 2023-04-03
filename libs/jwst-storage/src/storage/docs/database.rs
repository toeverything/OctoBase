use super::{entities::prelude::*, *};
use jwst::{sync_encode_update, DocStorage, Workspace};
use jwst_storage_migration::{Migrator, MigratorTrait};
use std::{
    collections::hash_map::Entry,
    panic::{catch_unwind, AssertUnwindSafe},
};
use yrs::{updates::decoder::Decode, Doc, ReadTxn, StateVector, Transact, Update};

const MAX_TRIM_UPDATE_LIMIT: u64 = 500;

fn migrate_update(updates: Vec<<Docs as EntityTrait>::Model>, doc: Doc) -> JwstResult<Doc> {
    {
        let mut trx = doc.transact_mut();
        for update in updates {
            let id = update.timestamp;
            match Update::decode_v1(&update.blob) {
                Ok(update) => {
                    if let Err(e) = catch_unwind(AssertUnwindSafe(|| trx.apply_update(update))) {
                        warn!("update {} merge failed, skip it: {:?}", id, e);
                    }
                }
                Err(err) => warn!("failed to decode update: {:?}", err),
            }
        }
        trx.commit();
    }

    trace!(
        "migrate_update: {:?}",
        doc.transact()
            .encode_state_as_update_v1(&StateVector::default())?
    );

    Ok(doc)
}

type DocsModel = <Docs as EntityTrait>::Model;
type DocsActiveModel = super::entities::docs::ActiveModel;
type DocsColumn = <Docs as EntityTrait>::Column;

pub struct DocDBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
    workspaces: RwLock<HashMap<String, Workspace>>,
    remote: RwLock<HashMap<String, Sender<Vec<u8>>>>,
}

impl DocDBStorage {
    pub async fn init_with_pool(pool: DatabaseConnection, bucket: Arc<Bucket>) -> JwstResult<Self> {
        Migrator::up(&pool, None)
            .await
            .context("failed to run migration")?;

        Ok(Self {
            bucket,
            pool,
            workspaces: RwLock::new(HashMap::new()),
            remote: RwLock::new(HashMap::new()),
        })
    }

    pub async fn init_pool(database: &str) -> JwstResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite)).await
    }

    pub fn remote(&self) -> &RwLock<HashMap<String, Sender<Vec<u8>>>> {
        &self.remote
    }

    async fn all<C>(conn: &C, table: &str) -> JwstResult<Vec<DocsModel>>
    where
        C: ConnectionTrait,
    {
        trace!("start scan all: {table}");
        let models = Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .all(conn)
            .await
            .context("failed to scan all updates")?;
        trace!("end scan all: {table}, {}", models.len());
        Ok(models)
    }

    async fn count<C>(conn: &C, table: &str) -> JwstResult<u64>
    where
        C: ConnectionTrait,
    {
        trace!("start count: {table}");
        let count = Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .count(conn)
            .await
            .context("failed to count update")?;
        trace!("end count: {table}, {count}");
        Ok(count)
    }

    async fn insert<C>(conn: &C, table: &str, blob: &[u8]) -> JwstResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start insert: {table}");
        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob.into()),
            ..Default::default()
        })
        .exec(conn)
        .await
        .context("failed to insert update")?;
        trace!("end insert: {table}");
        Ok(())
    }

    async fn replace_with<C>(conn: &C, table: &str, blob: Vec<u8>) -> JwstResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start replace: {table}");
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(conn)
            .await
            .context("failed to delete old updates")?;
        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob),
            ..Default::default()
        })
        .exec(conn)
        .await
        .context("failed to insert new updates")?;
        trace!("end replace: {table}");
        Ok(())
    }

    async fn drop<C>(conn: &C, table: &str) -> JwstResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start drop: {table}");
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(conn)
            .await
            .context("failed to delete updates")?;
        trace!("end drop: {table}");
        Ok(())
    }

    async fn update<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> JwstResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start update: {table}");
        let update_size = Self::count(conn, table).await?;
        if update_size > MAX_TRIM_UPDATE_LIMIT - 1 {
            trace!("full migrate update: {table}, {update_size}");
            let data = Self::all(conn, table).await?;

            let data = tokio::task::spawn_blocking(move || {
                migrate_update(data, Doc::default())
                    .and_then(|doc| {
                        Ok(doc
                            .transact()
                            .encode_state_as_update_v1(&StateVector::default())?)
                    })
                    .expect("failed to encode update")
            })
            .await
            .context("failed to merge update")?;

            Self::replace_with(conn, table, data).await?;
        } else {
            trace!("insert update: {table}, {update_size}");
            Self::insert(conn, table, &blob).await?;
        }
        trace!("end update: {table}");

        trace!("update {}bytes to {}", blob.len(), table);
        if let Entry::Occupied(remote) = self.remote.write().await.entry(table.into()) {
            let broadcast = &remote.get();
            if broadcast.send(sync_encode_update(&blob)).is_err() {
                // broadcast failures are not fatal errors, only warnings are required
                warn!("send {table} update to pipeline failed");
            }
        }
        trace!("end update broadcast: {table}");

        Ok(())
    }

    async fn full_migrate<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> JwstResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start full migrate: {table}");
        if Self::count(conn, table).await? > 0 {
            Self::replace_with(conn, table, blob).await?;
        } else {
            Self::insert(conn, table, &blob).await?;
        }
        trace!("end full migrate: {table}");
        Ok(())
    }

    async fn create_doc<C>(conn: &C, workspace: &str) -> JwstResult<Doc>
    where
        C: ConnectionTrait,
    {
        trace!("start create doc: {workspace}");
        let mut doc = Doc::new();

        let all_data = Self::all(conn, workspace).await?;

        if all_data.is_empty() {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default())?;
            Self::insert(conn, workspace, &update).await?;
        } else {
            doc = migrate_update(all_data, doc)?;
        }
        trace!("end create doc: {workspace}");

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage for DocDBStorage {
    async fn exists(&self, workspace_id: String) -> JwstResult<bool> {
        trace!("check workspace exists: get lock");
        let _lock = self.bucket.read().await;

        Ok(self.workspaces.read().await.contains_key(&workspace_id)
            || Self::count(&self.pool, &workspace_id)
                .await
                .map(|c| c > 0)
                .context("failed to check workspace")
                .map_err(JwstError::StorageError)?)
    }

    async fn get(&self, workspace_id: String) -> JwstResult<Workspace> {
        trace!("get workspace: enter");
        match self.workspaces.write().await.entry(workspace_id.clone()) {
            Entry::Occupied(ws) => {
                trace!("get workspace cache: {workspace_id}");
                Ok(ws.get().clone())
            }
            Entry::Vacant(v) => {
                debug!("init workspace cache: get lock");
                let _lock = self.bucket.write().await;
                info!("init workspace cache: {workspace_id}");
                let id = workspace_id.clone();
                let doc = Self::create_doc(&self.pool, &id)
                    .await
                    .context("failed to create workspace")
                    .map_err(JwstError::StorageError)?;

                let ws = Workspace::from_doc(doc, workspace_id);
                Ok(v.insert(ws).clone())
            }
        }
    }

    async fn write_full_update(&self, workspace_id: String, data: Vec<u8>) -> JwstResult<()> {
        trace!("write_full_update: get lock");
        let _lock = self.bucket.write().await;

        trace!("write_doc: {:?}", data);

        self.full_migrate(&self.pool, &workspace_id, data)
            .await
            .context("Failed to store workspace")
            .map_err(JwstError::StorageError)?;

        debug_assert_eq!(Self::count(&self.pool, &workspace_id).await?, 1u64);

        Ok(())
    }

    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<()> {
        debug!("write_update: get lock");
        let _lock = self.bucket.write().await;

        trace!("write_update: {:?}", data);
        self.update(&self.pool, &workspace_id, data.into())
            .await
            .context("failed to store update workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }

    async fn delete(&self, workspace_id: String) -> JwstResult<()> {
        debug!("delete workspace: get lock");
        let _lock = self.bucket.write().await;

        debug!("delete workspace cache: {workspace_id}");
        self.workspaces.write().await.remove(&workspace_id);
        DocDBStorage::drop(&self.pool, &workspace_id)
            .await
            .context("failed to delete workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }
}

#[cfg(test)]
pub async fn docs_storage_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    let conn = &pool.pool;

    DocDBStorage::drop(conn, "basic").await?;

    // empty table
    assert_eq!(DocDBStorage::count(conn, "basic").await?, 0);

    // first insert
    DocDBStorage::insert(conn, "basic", &[1, 2, 3, 4]).await?;
    DocDBStorage::insert(conn, "basic", &[2, 2, 3, 4]).await?;
    assert_eq!(DocDBStorage::count(conn, "basic").await?, 2);

    // second insert
    DocDBStorage::replace_with(conn, "basic", vec![3, 2, 3, 4]).await?;

    let all = DocDBStorage::all(conn, "basic").await?;
    assert_eq!(
        all,
        vec![DocsModel {
            id: all.get(0).unwrap().id,
            workspace: "basic".into(),
            timestamp: all.get(0).unwrap().timestamp,
            blob: vec![3, 2, 3, 4]
        }]
    );
    assert_eq!(DocDBStorage::count(conn, "basic").await?, 1);

    DocDBStorage::drop(conn, "basic").await?;

    DocDBStorage::insert(conn, "basic", &[1, 2, 3, 4]).await?;

    let all = DocDBStorage::all(conn, "basic").await?;
    assert_eq!(
        all,
        vec![DocsModel {
            id: all.get(0).unwrap().id,
            workspace: "basic".into(),
            timestamp: all.get(0).unwrap().timestamp,
            blob: vec![1, 2, 3, 4]
        }]
    );
    assert_eq!(DocDBStorage::count(conn, "basic").await?, 1);

    Ok(())
}

#[cfg(test)]
#[cfg(feature = "postgres")]
pub async fn full_migration_stress_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    let final_bytes: Vec<u8> = (0..1024 * 100).map(|_| rand::random::<u8>()).collect();
    for i in 0..=50 {
        let random_bytes: Vec<u8> = if i == 50 {
            final_bytes.clone()
        } else {
            (0..1024 * 100).map(|_| rand::random::<u8>()).collect()
        };
        let (r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15) = tokio::join!(
            pool.write_full_update("full_migration_1".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_2".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_3".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_4".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_5".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_6".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_7".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_8".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_9".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_10".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_11".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_12".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_13".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_14".to_owned(), random_bytes.clone()),
            pool.write_full_update("full_migration_15".to_owned(), random_bytes.clone())
        );
        r1?;
        r2?;
        r3?;
        r4?;
        r5?;
        r6?;
        r7?;
        r8?;
        r9?;
        r10?;
        r11?;
        r12?;
        r13?;
        r14?;
        r15?;
    }

    assert_eq!(
        DocDBStorage::all(&pool.pool, "full_migration_1")
            .await?
            .into_iter()
            .map(|d| d.blob)
            .collect::<Vec<_>>(),
        vec![final_bytes.clone()]
    );

    assert_eq!(
        DocDBStorage::all(&pool.pool, "full_migration_2")
            .await?
            .into_iter()
            .map(|d| d.blob)
            .collect::<Vec<_>>(),
        vec![final_bytes]
    );

    Ok(())
}

#[cfg(test)]
pub async fn docs_storage_partial_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    use tokio::sync::mpsc::channel;

    let conn = &pool.pool;

    DocDBStorage::drop(conn, "basic").await?;

    // empty table
    assert_eq!(DocDBStorage::count(conn, "basic").await?, 0);

    {
        let ws = pool.get("basic".into()).await.unwrap();

        let (tx, mut rx) = channel(100);

        let sub = ws
            .doc()
            .observe_update_v1(move |_, e| {
                futures::executor::block_on(async {
                    tx.send(e.update.clone()).await.unwrap();
                });
            })
            .unwrap();

        ws.with_trx(|mut t| {
            let space = t.get_space("test");
            let block = space.create(&mut t.trx, "block1", "text").unwrap();
            block.set(&mut t.trx, "test1", "value1").unwrap();
        });

        ws.with_trx(|mut t| {
            let space = t.get_space("test");
            let block = space.get(&mut t.trx, "block1").unwrap();
            block.set(&mut t.trx, "test2", "value2").unwrap();
        });

        ws.with_trx(|mut t| {
            let space = t.get_space("test");
            let block = space.create(&mut t.trx, "block2", "block2").unwrap();
            block.set(&mut t.trx, "test3", "value3").unwrap();
        });

        drop(sub);

        while let Some(update) = rx.recv().await {
            info!("recv: {}", update.len());
            pool.write_update("basic".into(), &update).await.unwrap();
        }

        assert_eq!(DocDBStorage::count(conn, "basic").await?, 4);
    }

    pool.workspaces.write().await.clear();

    {
        let ws = pool.get("basic".into()).await.unwrap();
        ws.with_trx(|mut t| {
            let space = t.get_space("test");

            let block = space.get(&mut t.trx, "block1").unwrap();
            assert_eq!(block.flavour(&t.trx), "text");
            assert_eq!(block.get(&t.trx, "test1"), Some("value1".into()));
            assert_eq!(block.get(&t.trx, "test2"), Some("value2".into()));

            let block = space.get(&mut t.trx, "block2").unwrap();
            assert_eq!(block.flavour(&t.trx), "block2");
            assert_eq!(block.get(&t.trx, "test3"), Some("value3".into()));
        });
    }

    Ok(())
}
