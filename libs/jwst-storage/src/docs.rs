use super::{entities::prelude::*, *};
use dashmap::{mapref::entry::Entry, DashMap};
use futures::{SinkExt, StreamExt};
use jwst::{info, sync_encode_update, DocStorage, DocSync, Workspace};
use jwst_storage_migration::{Migrator, MigratorTrait};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};
use tokio::sync::{
    mpsc::{channel, Sender},
    RwLock,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};
use url::Url;
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

#[derive(Clone)]
pub struct DocAutoStorage {
    pool: DatabaseConnection,
    workspaces: DashMap<String, Arc<RwLock<Workspace>>>,
    remote: DashMap<String, Sender<Message>>,
}

impl DocAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection) -> Result<Self, DbErr> {
        Migrator::up(&pool, None).await?;
        Ok(Self {
            pool,
            workspaces: DashMap::new(),
            remote: DashMap::new(),
        })
    }

    pub async fn init_pool(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;
        Migrator::up(&pool, None).await?;
        Ok(Self {
            pool,
            workspaces: DashMap::new(),
            remote: DashMap::new(),
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

    pub async fn all(&self, table: &str) -> Result<Vec<DocsModel>, DbErr> {
        Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .all(&self.pool)
            .await
    }

    pub(super) async fn count(&self, table: &str) -> Result<u64, DbErr> {
        Docs::find()
            .filter(DocsColumn::Workspace.eq(table))
            .count(&self.pool)
            .await
    }

    pub async fn insert(&self, table: &str, blob: &[u8]) -> Result<(), DbErr> {
        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob.into()),
            ..Default::default()
        })
        .exec(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn replace_with(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        let tx = self.pool.begin().await?;

        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(&tx)
            .await?;

        Docs::insert(DocsActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now().into()),
            blob: Set(blob),
            ..Default::default()
        })
        .exec(&tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn drop(&self, table: &str) -> Result<(), DbErr> {
        Docs::delete_many()
            .filter(DocsColumn::Workspace.eq(table))
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        if self.count(table).await? > MAX_TRIM_UPDATE_LIMIT - 1 {
            let data = self.all(table).await?;

            let doc = migrate_update(data, Doc::default());

            let data = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());

            self.replace_with(table, data).await?;
        } else {
            self.insert(table, &blob).await?;
        }

        debug!("update {}bytes to {}", blob.len(), table);
        if let Entry::Occupied(remote) = self.remote.entry(table.into()) {
            let socket = &remote.get();
            debug!("send update to channel");
            if let Err(e) = socket
                .send(Message::binary(sync_encode_update(&blob)))
                .await
            {
                warn!("send update to remote failed: {:?}", e);
                socket.closed().await;
            }
            debug!("send update to channel end");
        }

        Ok(())
    }

    pub async fn full_migrate(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        if self.count(table).await? > 0 {
            self.replace_with(table, blob).await
        } else {
            self.insert(table, &blob).await
        }
    }

    async fn create_doc(&self, workspace: &str) -> Result<Doc, DbErr> {
        let mut doc = Doc::with_options(Options {
            skip_gc: true,
            ..Default::default()
        });

        let all_data = self.all(workspace).await?;

        if all_data.is_empty() {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());
            self.insert(workspace, &update).await?;
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
                .count(&workspace_id)
                .await
                .map(|c| c > 0)
                .context("Failed to check workspace")
                .map_err(JwstError::StorageError)?)
    }

    async fn get(&self, workspace_id: String) -> JwstResult<Arc<RwLock<Workspace>>> {
        match self.workspaces.entry(workspace_id.clone()) {
            Entry::Occupied(ws) => Ok(ws.get().clone()),
            Entry::Vacant(v) => {
                debug!("init workspace cache");
                let doc = self
                    .create_doc(&workspace_id)
                    .await
                    .context("Failed to check workspace")
                    .map_err(JwstError::StorageError)?;

                let ws = Arc::new(RwLock::new(Workspace::from_doc(doc, workspace_id)));
                Ok(v.insert(ws).clone())
            }
        }
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_full_update(&self, workspace_id: String, data: Vec<u8>) -> JwstResult<()> {
        trace!("write_doc: {:?}", data);

        Ok(self
            .full_migrate(&workspace_id, data)
            .await
            .context("Failed to store workspace")
            .map_err(JwstError::StorageError)?)
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<()> {
        trace!("write_update: {:?}", data);
        self.update(&workspace_id, data.into())
            .await
            .context("Failed to store update workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }

    async fn delete(&self, workspace_id: String) -> JwstResult<()> {
        self.workspaces.remove(&workspace_id);
        self.drop(&workspace_id)
            .await
            .context("Failed to delete workspace")
            .map_err(JwstError::StorageError)?;

        Ok(())
    }
}

#[async_trait]
impl DocSync for DocAutoStorage {
    async fn sync(&self, id: String, remote: String) -> JwstResult<Arc<RwLock<Workspace>>> {
        let workspace = self.get(id.clone()).await?;

        if let Entry::Vacant(entry) = self.remote.entry(id.clone()) {
            let (tx, mut rx) = channel(100);
            let workspace = workspace.clone();

            debug!("spawn sync thread");
            std::thread::spawn(move || {
                let Ok(rt) = tokio::runtime::Runtime::new() else {
                    return error!("Failed to create runtime");
                };
                rt.block_on(async move {
                    debug!("generate remote config");
                    let uri = match Url::parse(&remote) {
                        Ok(uri) => uri,
                        Err(e) => {
                            return error!("Failed to parse remote url: {}", e);
                        }
                    };
                    let mut req = match uri.into_client_request() {
                        Ok(req)=> req,
                        Err(e) => {
                            return error!("Failed to create client request: {}", e);
                        }
                    };
                    req.headers_mut()
                        .append("Sec-WebSocket-Protocol", HeaderValue::from_static("AFFiNE"));

                    debug!("connect to remote: {}", req.uri());
                    let (mut socket_tx, mut socket_rx) = match connect_async(req).await {
                        Ok((socket, _)) => socket.split(),
                        Err(e) => {
                            return error!("Failed to connect to remote: {}", e);
                        }
                    };


                    debug!("sync init message");
                    match workspace.read().await.sync_init_message() {
                        Ok(init_data) => {
                            debug!("send init message");
                            if let Err(e) = socket_tx.send(Message::Binary(init_data)).await {
                                error!("Failed to send init message: {}", e);
                                return;
                            }
                        }
                        Err(e) => {
                            error!("Failed to create init message: {}", e);
                            return;
                        }
                    }

                    let id = workspace.read().await.id();
                    debug!("start sync thread {id}");
                    loop {
                        tokio::select! {
                            Some(msg) = socket_rx.next() => {
                                match msg {
                                    Ok(msg) => {
                                        if let Message::Binary(msg) = msg {
                                            trace!("get update from remote: {:?}", msg);
                                            let buffer = workspace.write().await.sync_decode_message(&msg);
                                            for update in buffer {
                                                // skip empty updates
                                                if update == [0, 2, 2, 0, 0] {
                                                    continue;
                                                }
                                                trace!("send differential update to remote: {:?}", update);
                                                if let Err(e) = socket_tx.send(Message::binary(update)).await {
                                                    warn!("send update to remote failed: {:?}", e);
                                                    if let Err(e) = socket_tx.close().await {
                                                        error!("close failed: {}", e);
                                                    };
                                                    break;
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        error!("remote closed: {e}");
                                        break;
                                    },
                                }
                            }
                            Some(msg) = rx.recv() => {
                                trace!("send update to remote: {:?}", msg);
                                if let Err(e) = socket_tx.send(msg).await {
                                    warn!("send update to remote failed: {:?}", e);
                                    if let Err(e) = socket_tx.close().await{
                                        error!("close failed: {}", e);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    debug!("end sync thread");
                });
            });

            entry.insert(tx);
        }

        Ok(workspace)
    }
}
