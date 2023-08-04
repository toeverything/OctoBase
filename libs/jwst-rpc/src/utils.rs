use super::*;
use jwst::{DocStorage, Workspace};
use jwst_codec::Doc;
use jwst_storage::{BlobStorageType, JwstStorage};
use nanoid::nanoid;
use std::{collections::HashMap, thread::JoinHandle as StdJoinHandler, time::Duration};
use tokio::{
    runtime::Runtime,
    sync::{mpsc::channel, RwLock},
    task::JoinHandle as TokioJoinHandler,
    time::sleep,
};
use yrs::{ReadTxn, StateVector, Transact};

pub struct MinimumServerContext {
    channel: BroadcastChannels,
    storage: JwstStorage,
}

// just for test
impl MinimumServerContext {
    pub async fn new() -> Arc<Self> {
        let storage = 'connect: loop {
            let mut retry = 3;
            match JwstStorage::new_with_migration(
                &std::env::var("DATABASE_URL")
                    .map(|url| format!("{url}_binary"))
                    .unwrap_or("sqlite::memory:".into()),
                BlobStorageType::DB,
            )
            .await
            {
                Ok(storage) => break 'connect Ok(storage),
                Err(e) => {
                    retry -= 1;
                    if retry > 0 {
                        error!("failed to connect database: {}", e);
                        sleep(Duration::from_secs(1)).await;
                    } else {
                        break 'connect Err(e);
                    }
                }
            }
        }
        .unwrap();

        Arc::new(Self {
            channel: RwLock::new(HashMap::new()),
            storage,
        })
    }

    pub async fn new_with_workspace(
        workspace_id: &str,
    ) -> (Arc<MinimumServerContext>, Workspace, Vec<u8>) {
        let server = Self::new().await;
        server
            .get_storage()
            .docs()
            .delete_workspace(workspace_id)
            .await
            .unwrap();
        let ws = server.get_workspace(workspace_id).await.unwrap();

        let init_state = ws
            .doc()
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
            .expect("encode_state_as_update_v1 failed");

        (server, ws, init_state)
    }
}

impl RpcContextImpl<'_> for MinimumServerContext {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}

pub async fn connect_memory_workspace(
    server: Arc<MinimumServerContext>,
    init_state: &[u8],
    id: &str,
) -> (
    Doc,
    Sender<Message>,
    TokioJoinHandler<()>,
    StdJoinHandler<()>,
    Arc<Runtime>,
) {
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

    let mut doc = Doc::default();
    doc.apply_update_from_binary(init_state.to_vec()).unwrap();

    let (tx, rx, tx_handler, rx_handler) = memory_connector(rt.clone(), doc.clone());
    {
        let (last_synced_tx, mut last_synced_rx) = channel::<i64>(128);
        let tx = tx.clone();
        let workspace_id = id.to_string();
        {
            let rt = rt.clone();
            std::thread::spawn(move || {
                rt.block_on(handle_connector(
                    server,
                    workspace_id,
                    nanoid!(),
                    move || (tx, rx, last_synced_tx),
                ));
            });
        }

        let success = last_synced_rx.recv().await;

        if success.unwrap_or(0) > 0 {
            info!("{id} first init success");
        } else {
            error!("{id} first init failed");
        }

        rt.spawn(async move {
            while let Some(last_synced) = last_synced_rx.recv().await {
                info!("last synced: {}", last_synced);
            }
        });
    }

    (doc, tx, tx_handler, rx_handler, rt)
}
