use super::{async_trait, entities::prelude::*, DocStorage, DocSync};
use chrono::Utc;
use dashmap::{mapref::entry::Entry, DashMap};
use futures::{SinkExt, StreamExt};
use jwst::{sync_encode_update, Workspace};
use jwst_doc_migration::{Migrator, MigratorTrait};
use jwst_logger::{debug, error, trace, warn};
use path_ext::PathExt;
use sea_orm::{prelude::*, Database, Set, TransactionTrait};
use std::{
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
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
use yrs::{updates::decoder::Decode, Doc, Options, StateVector, Update};

const MAX_TRIM_UPDATE_LIMIT: u64 = 500;

fn migrate_update(updates: Vec<<UpdateBinary as EntityTrait>::Model>, doc: Doc) -> Doc {
    let mut trx = doc.transact();
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

    trace!(
        "migrate_update: {:?}",
        doc.encode_state_as_update_v1(&StateVector::default())
    );

    doc
}

type UpdateBinaryModel = <UpdateBinary as EntityTrait>::Model;
type UpdateBinaryActiveModel = super::entities::update_binary::ActiveModel;
type UpdateBinaryColumn = <UpdateBinary as EntityTrait>::Column;

pub struct DocAutoStorage {
    pool: DatabaseConnection,
    workspaces: DashMap<String, Arc<RwLock<Workspace>>>,
    remote: DashMap<String, Sender<Message>>,
}

impl DocAutoStorage {
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

    pub async fn all(&self, table: &str) -> Result<Vec<UpdateBinaryModel>, DbErr> {
        UpdateBinary::find()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .all(&self.pool)
            .await
    }

    async fn count(&self, table: &str) -> Result<u64, DbErr> {
        UpdateBinary::find()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .count(&self.pool)
            .await
    }

    async fn insert(&self, table: &str, blob: &[u8]) -> Result<(), DbErr> {
        UpdateBinary::insert(UpdateBinaryActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now()),
            blob: Set(blob.into()),
        })
        .exec(&self.pool)
        .await?;
        Ok(())
    }

    async fn replace_with(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        let mut tx = self.pool.begin().await?;

        UpdateBinary::delete_many()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .exec(&mut tx)
            .await?;

        UpdateBinary::insert(UpdateBinaryActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now()),
            blob: Set(blob.into()),
        })
        .exec(&mut tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn drop(&self, table: &str) -> Result<(), DbErr> {
        UpdateBinary::delete_many()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        if self.count(table).await? > MAX_TRIM_UPDATE_LIMIT - 1 {
            let data = self.all(table).await?;

            let doc = migrate_update(data, Doc::default());

            let data = doc.encode_state_as_update_v1(&StateVector::default());

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
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            self.insert(workspace, &update).await?;
        } else {
            doc = migrate_update(all_data, doc);
        }

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage for DocAutoStorage {
    async fn get(&self, workspace_id: String) -> io::Result<Arc<RwLock<Workspace>>> {
        match self.workspaces.entry(workspace_id.clone()) {
            Entry::Occupied(ws) => Ok(ws.get().clone()),
            Entry::Vacant(v) => {
                let doc = self
                    .create_doc(&workspace_id)
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                let ws = Arc::new(RwLock::new(Workspace::from_doc(doc, workspace_id)));
                Ok(v.insert(ws).clone())
            }
        }
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_doc(&self, workspace_id: String, doc: &Doc) -> io::Result<()> {
        let data = doc.encode_state_as_update_v1(&StateVector::default());
        trace!("write_doc: {:?}", data);

        self.full_migrate(&workspace_id, data)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> io::Result<bool> {
        trace!("write_update: {:?}", data);
        self.update(&workspace_id, data.into())
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(true)
    }

    async fn delete(&self, workspace_id: String) -> io::Result<()> {
        self.workspaces.remove(&workspace_id);
        self.drop(&workspace_id)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

#[async_trait]
impl DocSync for DocAutoStorage {
    async fn sync(&self, id: String, remote: String) -> io::Result<()> {
        if let Entry::Vacant(entry) = self.remote.entry(id.clone()) {
            let workspace = self.get(id).await.map_err(|e| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    format!("Failed to get doc: {}", e),
                )
            })?;

            let (tx, mut rx) = channel(100);

            debug!("spawn sync thread");
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(async move {
                    debug!("generate remote config");
                    let mut req = Url::parse(&remote)
                        .expect( "Failed to parse remote url")
                        .into_client_request()
                        .expect("Failed to create client request");
                    req.headers_mut()
                        .append("Sec-WebSocket-Protocol", HeaderValue::from_static("AFFiNE"));

                    debug!("connect to remote: {}", req.uri());
                    let (socket, _) = connect_async(req)
                        .await.expect("Failed to connect to remote");
                    let (mut socket_tx, mut socket_rx) = socket.split();

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
                                                    socket_tx.close().await.expect("close failed");
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
                                    socket_tx.close().await.expect("close failed");
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        let pool = DocAutoStorage::init_pool("sqlite::memory:").await?;

        // empty table
        assert_eq!(pool.count("basic").await?, 0);

        // first insert
        pool.insert("basic", &[1, 2, 3, 4]).await?;
        pool.insert("basic", &[2, 2, 3, 4]).await?;
        assert_eq!(pool.count("basic").await?, 2);

        // second insert
        pool.replace_with("basic", vec![3, 2, 3, 4]).await?;

        let all = pool.all("basic").await?;
        assert_eq!(
            all,
            vec![UpdateBinaryModel {
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![3, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        pool.drop("basic").await?;

        pool.insert("basic", &[1, 2, 3, 4]).await?;

        let all = pool.all("basic").await?;
        assert_eq!(
            all,
            vec![UpdateBinaryModel {
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        Ok(())
    }
}
