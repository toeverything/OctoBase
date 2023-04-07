use super::{entities::prelude::*, *};
use crate::types::JwstStorageResult;
use jwst::{sync_encode_update, DocStorage, Workspace};
use jwst_storage_migration::{Migrator, MigratorTrait};
use std::collections::hash_map::Entry;
use yrs::{Doc, ReadTxn, StateVector, Transact};

const MAX_TRIM_UPDATE_LIMIT: u64 = 500;

type DocsModel = <Docs as EntityTrait>::Model;
type DocsActiveModel = super::entities::docs::ActiveModel;
type DocsColumn = <Docs as EntityTrait>::Column;

pub struct DocDBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
    workspaces: RwLock<HashMap<String, Workspace>>, // memory cache
    remote: RwLock<HashMap<String, Sender<Vec<u8>>>>,
}

impl DocDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
    ) -> JwstStorageResult<Self> {
        Migrator::up(&pool, None).await?;

        Ok(Self {
            bucket,
            pool,
            workspaces: RwLock::new(HashMap::new()),
            remote: RwLock::new(HashMap::new()),
        })
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite)).await
    }

    pub fn remote(&self) -> &RwLock<HashMap<String, Sender<Vec<u8>>>> {
        &self.remote
    }

    async fn all<C>(conn: &C, workspace: &str) -> JwstStorageResult<Vec<DocsModel>>
    where
        C: ConnectionTrait,
    {
        trace!("start scan all: {workspace}");
        let models = Docs::find()
            .filter(DocsColumn::Workspace.eq(workspace))
            .all(conn)
            .await?;
        trace!("end scan all: {workspace}, {}", models.len());
        Ok(models)
    }

    async fn count<C>(conn: &C, workspace: &str) -> JwstStorageResult<u64>
    where
        C: ConnectionTrait,
    {
        trace!("start count: {workspace}");
        let count = Docs::find()
            .filter(DocsColumn::Workspace.eq(workspace))
            .count(conn)
            .await?;
        trace!("end count: {workspace}, {count}");
        Ok(count)
    }

    async fn insert<C>(conn: &C, workspace: &str, blob: &[u8]) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start insert: {workspace}");
        Docs::insert(DocsActiveModel {
            workspace: Set(workspace.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob.into()),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        trace!("end insert: {workspace}");
        Ok(())
    }

    async fn replace_with<C>(conn: &C, workspace: &str, blob: Vec<u8>) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start replace: {workspace}");
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(workspace))
            .exec(conn)
            .await?;
        Docs::insert(DocsActiveModel {
            workspace: Set(workspace.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        trace!("end replace: {workspace}");
        Ok(())
    }

    async fn delete<C>(conn: &C, workspace: &str) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start drop: {workspace}");
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(workspace))
            .exec(conn)
            .await?;
        trace!("end drop: {workspace}");
        Ok(())
    }

    async fn update<C>(&self, conn: &C, workspace: &str, blob: Vec<u8>) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start update: {workspace}");
        let update_size = Self::count(conn, workspace).await?;
        if update_size > MAX_TRIM_UPDATE_LIMIT - 1 {
            trace!("full migrate update: {workspace}, {update_size}");
            let doc_records = Self::all(conn, workspace).await?;

            let data = tokio::task::spawn_blocking(move || utils::merge_doc_records(doc_records))
                .await
                .map_err(JwstStorageError::DocMerge)??;

            Self::replace_with(conn, workspace, data).await?;
        } else {
            trace!("insert update: {workspace}, {update_size}");
            Self::insert(conn, workspace, &blob).await?;
        }
        trace!("end update: {workspace}");

        trace!("update {}bytes to {}", blob.len(), workspace);
        if let Entry::Occupied(remote) = self.remote.write().await.entry(workspace.into()) {
            let broadcast = &remote.get();
            if broadcast.send(sync_encode_update(&blob)).is_err() {
                // broadcast failures are not fatal errors, only warnings are required
                warn!("send {workspace} update to pipeline failed");
            }
        }
        trace!("end update broadcast: {workspace}");

        Ok(())
    }

    async fn full_migrate<C>(
        &self,
        conn: &C,
        workspace: &str,
        blob: Vec<u8>,
    ) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        trace!("start full migrate: {workspace}");
        if Self::count(conn, workspace).await? > 0 {
            Self::replace_with(conn, workspace, blob).await?;
        } else {
            Self::insert(conn, workspace, &blob).await?;
        }
        trace!("end full migrate: {workspace}");
        Ok(())
    }

    async fn create_doc<C>(conn: &C, workspace: &str) -> JwstStorageResult<Doc>
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
            doc = utils::migrate_update(all_data, doc)?;
        }
        trace!("end create doc: {workspace}");

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage<JwstStorageError> for DocDBStorage {
    async fn exists(&self, workspace_id: String) -> JwstStorageResult<bool> {
        trace!("check workspace exists: get lock");
        let _lock = self.bucket.read().await;

        Ok(self.workspaces.read().await.contains_key(&workspace_id)
            || Self::count(&self.pool, &workspace_id)
                .await
                .map(|c| c > 0)?)
    }

    async fn get(&self, workspace_id: String) -> JwstStorageResult<Workspace> {
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
                let doc = Self::create_doc(&self.pool, &id).await?;

                let ws = Workspace::from_doc(doc, workspace_id);
                Ok(v.insert(ws).clone())
            }
        }
    }

    async fn write_full_update(
        &self,
        workspace_id: String,
        data: Vec<u8>,
    ) -> JwstStorageResult<()> {
        trace!("write_full_update: get lock");
        let _lock = self.bucket.write().await;

        trace!("write_doc: {:?}", data);

        self.full_migrate(&self.pool, &workspace_id, data).await?;

        debug_assert_eq!(Self::count(&self.pool, &workspace_id).await?, 1u64);

        Ok(())
    }

    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstStorageResult<()> {
        debug!("write_update: get lock");
        let _lock = self.bucket.write().await;

        trace!("write_update: {:?}", data);
        self.update(&self.pool, &workspace_id, data.into()).await?;

        Ok(())
    }

    async fn delete(&self, workspace_id: String) -> JwstStorageResult<()> {
        debug!("delete workspace: get lock");
        let _lock = self.bucket.write().await;

        debug!("delete workspace cache: {workspace_id}");
        self.workspaces.write().await.remove(&workspace_id);
        DocDBStorage::delete(&self.pool, &workspace_id).await?;

        Ok(())
    }
}

#[cfg(test)]
pub async fn docs_storage_test(pool: &DocDBStorage) -> anyhow::Result<()> {
    let conn = &pool.pool;

    DocDBStorage::delete(conn, "basic").await?;

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

    DocDBStorage::delete(conn, "basic").await?;

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

    DocDBStorage::delete(conn, "basic").await?;

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

    // clear memory cache
    pool.workspaces.write().await.clear();

    {
        // memory cache empty, retrieve data from db
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
