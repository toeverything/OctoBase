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

pub(super) type DocsModel = <Docs as EntityTrait>::Model;
type DocsActiveModel = super::entities::docs::ActiveModel;
type DocsColumn = <Docs as EntityTrait>::Column;

pub struct DocAutoStorage {
    pub(super) pool: DatabaseConnection,
    workspaces: DashMap<String, Workspace>,
    remote: DashMap<String, Sender<Vec<u8>>>,
    pub(crate) last_migrate: DashMap<String, Instant>,
}

impl DocAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection) -> Result<Self, DbErr> {
        Migrator::up(&pool, None).await?;
        Ok(Self {
            pool,
            workspaces: DashMap::new(),
            remote: DashMap::new(),
            last_migrate: DashMap::new(),
        })
    }

    pub async fn init_pool(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;
        Migrator::up(&pool, None).await?;
        Ok(Self {
            pool,
            workspaces: DashMap::new(),
            remote: DashMap::new(),
            last_migrate: DashMap::new(),
        })
    }

    pub async fn init_sqlite_pool_with_name(file: &str) -> Result<Self, DbErr> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).map_err(|e| DbErr::Custom(e.to_string()))?;
        }

        Self::init_pool(&format!(
            "sqlite:{}?mode=rwc",
            data.join(PathBuf::from(file).name_str())
                .with_extension("db")
                .display()
        ))
        .await
    }

    pub async fn init_sqlite_pool_with_full_path(path: PathBuf) -> Result<Self, DbErr> {
        Self::init_pool(&format!("sqlite:{}?mode=rwc", path.display())).await
    }

    pub fn remote(&self) -> &DashMap<String, Sender<Vec<u8>>> {
        &&self.remote
    }

    pub(super) async fn all<C>(&self, conn: &C, table: &str) -> Result<Vec<DocsModel>, DbErr>
    where
        C: ConnectionTrait,
    {
        Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .all(conn)
            .await
    }

    pub(super) async fn count<C>(&self, conn: &C, table: &str) -> Result<u64, DbErr>
    where
        C: ConnectionTrait,
    {
        debug!("start count: {table}");
        let count = Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .count(conn)
            .await
            .unwrap();
        debug!("end count: {table}, {count}");
        Ok(count)
    }

    pub(super) async fn insert<C>(&self, conn: &C, table: &str, blob: &[u8]) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob.into()),
            ..Default::default()
        })
        .exec(conn)
        .await?;
        Ok(())
    }

    pub(super) async fn replace_with<C>(
        &self,
        conn: &C,
        table: &str,
        blob: Vec<u8>,
    ) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
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

        Ok(())
    }

    pub(super) async fn drop<C>(&self, conn: &C, table: &str) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(conn)
            .await?;
        Ok(())
    }

    async fn update<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
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

        debug!("update {}bytes to {}", blob.len(), table);
        if let Entry::Occupied(remote) = self.remote.entry(table.into()) {
            let broadcast = &remote.get();
            debug!("sending update to pipeline");
            if let Err(e) = broadcast.send(sync_encode_update(&blob)) {
                warn!("send update to pipeline failed: {:?}", e);
            }
            debug!("send update to pipeline end");
        }

        Ok(())
    }

    async fn full_migrate<C>(&self, conn: &C, table: &str, blob: Vec<u8>) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        info!("full migrate3.1: {table}");
        if self.count(conn, table).await? > 0 {
            info!("full migrate3.2: {table}");
            self.replace_with(conn, table, blob).await
        } else {
            info!("full migrate3.3: {table}");
            self.insert(conn, table, &blob).await
        }
    }

    async fn create_doc(&self, conn: &DatabaseTransaction, workspace: &str) -> Result<Doc, DbErr> {
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

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage for DocAutoStorage {
    async fn exists(&self, workspace_id: String) -> JwstResult<bool> {
        Ok(self.workspaces.contains_key(&workspace_id)
            || self
                .count(&self.pool, &workspace_id)
                .await
                .map(|c| c > 0)
                .context("Failed to check workspace")
                .map_err(JwstError::StorageError)?)
    }

    async fn get(&self, workspace_id: String) -> JwstResult<Workspace> {
        match self.workspaces.entry(workspace_id.clone()) {
            Entry::Occupied(ws) => Ok(ws.get().clone()),
            Entry::Vacant(v) => {
                debug!("init workspace cache: {workspace_id}");
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
        trace!("write_doc: {:?}", data);
        debug!("write_full_update 1");
        let trx = self
            .pool
            .begin()
            .await
            .context("failed to start transaction")?;

        debug!("write_full_update 2");
        self.full_migrate(&trx, &workspace_id, data)
            .await
            .context("Failed to store workspace")
            .map_err(JwstError::StorageError)?;

        debug!("write_full_update 3");
        trx.commit().await.context("failed to commit transaction")?;
        debug!("write_full_update 4");
        Ok(())
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<()> {
        trace!("write_update: {:?}", data);
        self.update(&self.pool, &workspace_id, data.into())
            .await
            .context("Failed to store update workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }

    async fn delete(&self, workspace_id: String) -> JwstResult<()> {
        debug!("delete workspace cache: {workspace_id}");
        self.workspaces.remove(&workspace_id);
        self.drop(&self.pool, &workspace_id)
            .await
            .context("Failed to delete workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }
}
