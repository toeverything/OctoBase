use super::{entities::prelude::*, *};
use dashmap::{mapref::entry::Entry, DashMap};
use jwst::{sync_encode_update, DocStorage, Workspace};
use jwst_storage_migration::{Migrator, MigratorTrait};
use sea_orm::DatabaseTransaction;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    time::Instant,
};
use tokio::sync::broadcast::Sender;
use yrs::{updates::decoder::Decode, Doc, Options, ReadTxn, StateVector, Transact, Update};

const MAX_TRIM_UPDATE_LIMIT: u64 = 500;

fn migrate_update(updates: Vec<<Docs as EntityTrait>::Model>, doc: Doc) -> Doc {
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
            .encode_state_as_update_v1(&StateVector::default())
    );

    doc
}

type DocsModel = <Docs as EntityTrait>::Model;
type DocsActiveModel = super::entities::docs::ActiveModel;
type DocsColumn = <Docs as EntityTrait>::Column;

pub struct DocAutoStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
    workspaces: DashMap<String, Workspace>,
    remote: DashMap<String, Sender<Vec<u8>>>,
    pub(crate) last_migrate: DashMap<String, Instant>,
}

impl DocAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection, bucket: Arc<Bucket>) -> JwstResult<Self> {
        Migrator::up(&pool, None)
            .await
            .context("failed to run migration")?;

        Ok(Self {
            bucket,
            pool,
            workspaces: DashMap::new(),
            remote: DashMap::new(),
            last_migrate: DashMap::new(),
        })
    }

    pub async fn init_pool(database: &str) -> JwstResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite)).await
    }

    pub async fn init_sqlite_pool_with_name(file: &str) -> JwstResult<Self> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data)?;
        }

        Self::init_pool(&format!(
            "sqlite:{}?mode=rwc",
            data.join(PathBuf::from(file).name_str())
                .with_extension("db")
                .display()
        ))
        .await
    }

    pub async fn init_sqlite_pool_with_full_path(path: PathBuf) -> JwstResult<Self> {
        Self::init_pool(&format!("sqlite:{}?mode=rwc", path.display())).await
    }

    pub fn remote(&self) -> &DashMap<String, Sender<Vec<u8>>> {
        &self.remote
    }

    async fn all<C>(&self, conn: &C, table: &str) -> Result<Vec<DocsModel>, DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start scan all: {table}");
        let models = Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .all(conn)
            .await?;
        info!("end scan all: {table}, {}", models.len());
        Ok(models)
    }

    async fn count<C>(&self, conn: &C, table: &str) -> Result<u64, DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start count: {table}");
        let count = Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .count(conn)
            .await
            .unwrap();
        info!("end count: {table}, {count}");
        Ok(count)
    }

    async fn insert<C>(&self, conn: &C, table: &str, blob: &[u8]) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start insert: {table}");
        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob.into()),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        info!("end insert: {table}");
        Ok(())
    }

    async fn replace_with<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start replace: {table}");
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(conn)
            .await?;
        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        info!("end replace: {table}");
        Ok(())
    }

    async fn drop<C>(&self, conn: &C, table: &str) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start drop: {table}");
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(conn)
            .await?;
        info!("end drop: {table}");
        Ok(())
    }

    async fn update<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start update: {table}");
        if self.count(conn, table).await? > MAX_TRIM_UPDATE_LIMIT - 1 {
            let data = self.all(conn, table).await?;

            let doc = migrate_update(data, Doc::default());

            let data = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());

            self.replace_with(conn, table, data).await?;
        } else {
            self.insert(conn, table, &blob).await?;
        }
        info!("end update: {table}");

        debug!("update {}bytes to {}", blob.len(), table);
        if let Entry::Occupied(remote) = self.remote.entry(table.into()) {
            let broadcast = &remote.get();
            debug!("sending update to pipeline");
            if let Err(e) = broadcast.send(sync_encode_update(&blob)) {
                warn!("send update to pipeline failed: {:?}", e);
            }
            debug!("send update to pipeline end");
        }
        info!("end update broadcast: {table}");

        Ok(())
    }

    async fn full_migrate<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start full migrate: {table}");
        if self.count(conn, table).await? > 0 {
            info!("full migrate1.1: {table}");
            self.replace_with(conn, table, blob).await?;
        } else {
            info!("full migrate1.2: {table}");
            self.insert(conn, table, &blob).await?;
        }
        info!("end full migrate: {table}");
        Ok(())
    }

    async fn create_doc<C>(&self, conn: &C, workspace: &str) -> Result<Doc, DbErr>
    where
        C: ConnectionTrait,
    {
        info!("start create doc: {workspace}");
        let mut doc = Doc::with_options(Options {
            skip_gc: true,
            ..Default::default()
        });

        let all_data = self.all(conn, workspace).await?;

        if all_data.is_empty() {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());
            self.insert(conn, workspace, &update).await?;
        } else {
            doc = migrate_update(all_data, doc);
        }
        info!("end create doc: {workspace}");

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage for DocAutoStorage {
    async fn exists(&self, workspace_id: String) -> JwstResult<bool> {
        info!("check workspace exists: get lock");
        self.bucket.until_ready().await;

        Ok(self.workspaces.contains_key(&workspace_id)
            || self
                .count(&self.pool, &workspace_id)
                .await
                .map(|c| c > 0)
                .context("Failed to check workspace")
                .map_err(JwstError::StorageError)?)
    }

    async fn get(&self, workspace_id: String) -> JwstResult<Workspace> {
        info!("get workspace: enter");
        match self.workspaces.entry(workspace_id.clone()) {
            Entry::Occupied(ws) => {
                info!("get workspace: get cached");
                Ok(ws.get().clone())
            }
            Entry::Vacant(v) => {
                info!("init workspace cache: get lock");
                self.bucket.until_ready().await;

                info!("init workspace cache: {workspace_id}");
                let trx = self
                    .pool
                    .begin()
                    .await
                    .context("failed to start transaction")?;
                let doc = self
                    .create_doc(&trx, &workspace_id)
                    .await
                    .context("failed to check workspace")
                    .map_err(JwstError::StorageError)?;
                trx.commit().await.context("failed to commit transaction")?;

                let ws = Workspace::from_doc(doc, workspace_id);
                Ok(v.insert(ws).clone())
            }
        }
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_full_update(&self, workspace_id: String, data: Vec<u8>) -> JwstResult<()> {
        info!("write_full_update: get lock");
        self.bucket.until_ready().await;

        trace!("write_doc: {:?}", data);
        info!("write_full_update 1");
        let trx = self
            .pool
            .begin()
            .await
            .context("failed to start transaction")?;

        info!("write_full_update 2");
        self.full_migrate(&trx, &workspace_id, data)
            .await
            .context("Failed to store workspace")
            .map_err(JwstError::StorageError)?;

        info!("write_full_update 3");
        trx.commit().await.context("failed to commit transaction")?;
        info!("write_full_update 4");
        Ok(())
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<()> {
        info!("write_update: get lock");
        self.bucket.until_ready().await;

        trace!("write_update: {:?}", data);
        self.update(&self.pool, &workspace_id, data.into())
            .await
            .context("Failed to store update workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }

    async fn delete(&self, workspace_id: String) -> JwstResult<()> {
        info!("delete workspace: get lock");
        self.bucket.until_ready().await;

        debug!("delete workspace cache: {workspace_id}");
        self.workspaces.remove(&workspace_id);
        self.drop(&self.pool, &workspace_id)
            .await
            .context("Failed to delete workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }
}

#[cfg(test)]
pub async fn docs_storage_test(pool: &DocAutoStorage) -> anyhow::Result<()> {
    let conn = &pool.pool;

    pool.drop(conn, "basic").await?;

    // empty table
    assert_eq!(pool.count(conn, "basic").await?, 0);

    // first insert
    pool.insert(conn, "basic", &[1, 2, 3, 4]).await?;
    pool.insert(conn, "basic", &[2, 2, 3, 4]).await?;
    assert_eq!(pool.count(conn, "basic").await?, 2);

    // second insert
    pool.replace_with(conn, "basic", vec![3, 2, 3, 4]).await?;

    let all = pool.all(conn, "basic").await?;
    assert_eq!(
        all,
        vec![DocsModel {
            id: all.get(0).unwrap().id,
            workspace: "basic".into(),
            timestamp: all.get(0).unwrap().timestamp,
            blob: vec![3, 2, 3, 4]
        }]
    );
    assert_eq!(pool.count(conn, "basic").await?, 1);

    pool.drop(conn, "basic").await?;

    pool.insert(conn, "basic", &[1, 2, 3, 4]).await?;

    let all = pool.all(conn, "basic").await?;
    assert_eq!(
        all,
        vec![DocsModel {
            id: all.get(0).unwrap().id,
            workspace: "basic".into(),
            timestamp: all.get(0).unwrap().timestamp,
            blob: vec![1, 2, 3, 4]
        }]
    );
    assert_eq!(pool.count(conn, "basic").await?, 1);

    Ok(())
}

#[cfg(test)]
#[cfg(feature = "postgres")]
pub async fn full_migration_test(pool: &DocAutoStorage) -> anyhow::Result<()> {
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
        pool.all(&pool.pool, "full_migration_1")
            .await?
            .into_iter()
            .map(|d| d.blob)
            .collect::<Vec<_>>(),
        vec![final_bytes.clone()]
    );

    assert_eq!(
        pool.all(&pool.pool, "full_migration_2")
            .await?
            .into_iter()
            .map(|d| d.blob)
            .collect::<Vec<_>>(),
        vec![final_bytes]
    );

    Ok(())
}

#[cfg(test)]
mod test {
    use super::{Arc, DashMap, Doc, Entry, Workspace};
    use rand::random;
    use std::collections::HashSet;
    use threadpool::ThreadPool;

    #[test]
    fn dashmap_capacity_test() -> anyhow::Result<()> {
        let workspaces: Arc<DashMap<String, Workspace>> = Arc::new(DashMap::new());
        let pool = ThreadPool::with_name("worker".into(), 10);
        let mut set = HashSet::new();

        for _ in 0..100000 {
            let id = random::<u64>().to_string();
            set.insert(id.clone());
            let workspaces = workspaces.clone();

            pool.execute(move || {
                let workspace = match workspaces.entry(id.clone()) {
                    Entry::Occupied(ws) => ws.get().clone(),
                    Entry::Vacant(v) => {
                        let ws = Workspace::from_doc(Doc::new(), id.clone());
                        v.insert(ws).clone()
                    }
                };
                assert_eq!(workspace.id(), id);
            });
        }

        pool.join();

        for id in set {
            let workspaces = workspaces.clone();
            pool.execute(move || {
                let workspace = workspaces.get(&id).unwrap();
                assert_eq!(workspace.id(), id);
            });
        }
        pool.join();

        Ok(())
    }
}
