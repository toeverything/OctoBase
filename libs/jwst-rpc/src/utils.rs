use super::*;
use jwst::{DocStorage, Workspace};
use jwst_storage::{BlobStorageType, JwstStorage};
use nanoid::nanoid;
use std::{collections::HashMap, thread::JoinHandle as StdJoinHandler, time::Duration};
use tokio::{
    sync::{mpsc::channel, RwLock},
    task::JoinHandle as TokioJoinHandler,
    time::sleep,
};
use yrs::{updates::decoder::Decode, Doc, ReadTxn, StateVector, Transact, Update};

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
    Workspace,
    Sender<Message>,
    TokioJoinHandler<()>,
    StdJoinHandler<()>,
) {
    let doc = Doc::new();
    doc.transact_mut()
        .apply_update(Update::decode_v1(init_state).unwrap());
    let workspace = Workspace::from_doc(doc, id);

    let doc = workspace.doc();

    let (tx, rx, tx_handler, rx_handler) = memory_connector(doc, rand::random::<usize>());
    {
        let (first_init_tx, mut first_init_rx) = channel::<bool>(10);
        let tx = tx.clone();
        let workspace_id = id.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(handle_connector(
                server,
                workspace_id,
                nanoid!(),
                move || (tx, rx, first_init_tx),
            ));
        });
        // tokio::spawn(handle_connector(server, id.into(), nanoid!(), move || {
        //     (tx, rx, first_init_tx)
        // }));

        let success = first_init_rx.recv().await;

        if success.unwrap_or(false) {
            info!("{id} first init success");
        } else {
            error!("{id} first init failed");
        }
    }

    (workspace, tx, tx_handler, rx_handler)
}
