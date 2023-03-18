mod broadcast;
mod client;
mod connector;
mod context;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use client::start_client;
pub use connector::{memory_connector, socket_connector};
pub use context::RpcContextImpl;

use jwst::{debug, error, info, trace, warn};
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};

#[derive(Debug)]
pub enum Message {
    Binary(Vec<u8>),
    Close,
    Ping,
}

pub async fn handle_connector(
    context: Arc<impl RpcContextImpl<'static> + Send + Sync + 'static>,
    workspace_id: String,
    identifier: String,
    get_channel: impl FnOnce() -> (Sender<Message>, Receiver<Vec<u8>>),
) {
    info!("{} collaborate with workspace {}", identifier, workspace_id);

    let (tx, rx) = get_channel();

    context
        .apply_change(&workspace_id, &identifier, tx.clone(), rx)
        .await;

    let mut ws = context
        .get_workspace(&workspace_id)
        .await
        .expect("failed to get workspace");

    let mut broadcast_update = context.join_broadcast(&mut ws).await;
    let mut server_update = context.join_server_broadcast(&workspace_id).await;

    if let Ok(init_data) = ws.sync_init_message().await {
        if tx.send(Message::Binary(init_data)).await.is_err() {
            // client disconnected
            if let Err(e) = tx.send(Message::Close).await {
                error!("failed to send close event: {}", e);
            }
            return;
        }
    } else {
        if let Err(e) = tx.send(Message::Close).await {
            error!("failed to send close event: {}", e);
        }
        return;
    }

    'sync: loop {
        tokio::select! {
            Ok(msg) = server_update.recv()=> {
                let ts = Instant::now();
                trace!("recv from server update: {:?}", msg);
                if tx.send(Message::Binary(msg.clone())).await.is_err() {
                    // pipeline was closed
                    break 'sync;
                }
                if ts.elapsed().as_micros() > 100 {
                    debug!("process server update cost: {}ms", ts.elapsed().as_micros());
                }

            },
            Ok(msg) = broadcast_update.recv()=> {
                let ts = Instant::now();
                match msg {
                    BroadcastType::BroadcastAwareness(data) => {
                        let ts = Instant::now();
                        trace!(
                            "recv awareness update from broadcast: {:?}bytes",
                            data.len()
                        );
                        if tx.send(Message::Binary(data.clone())).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!(
                                "process broadcast awareness cost: {}ms",
                                ts.elapsed().as_micros()
                            );
                        }
                    }
                    BroadcastType::BroadcastContent(data) => {
                        let ts = Instant::now();
                        trace!("recv content update from broadcast: {:?}bytes", data.len());
                        if tx.send(Message::Binary(data.clone())).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!(
                                "process broadcast content cost: {}ms",
                                ts.elapsed().as_micros()
                            );
                        }
                    }
                    BroadcastType::CloseUser(user) if user == identifier => {
                        let ts = Instant::now();
                        if tx.send(Message::Close).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!("process close user cost: {}ms", ts.elapsed().as_micros());
                        }

                        break;
                    }
                    BroadcastType::CloseAll => {
                        let ts = Instant::now();
                        if tx.send(Message::Close).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!("process close all cost: {}ms", ts.elapsed().as_micros());
                        }

                        break 'sync;
                    }
                    _ => {}
                }

                if ts.elapsed().as_micros() > 100 {
                    debug!("process broadcast cost: {}ms", ts.elapsed().as_micros());
                }
            },
            _ = sleep(Duration::from_secs(5)) => {
                context
                    .get_storage()
                    .full_migrate(workspace_id.clone(), None, false)
                    .await;
                if tx.is_closed() || tx.send(Message::Ping).await.is_err() {
                    break 'sync;
                }
            }
        }
    }

    // make a final store
    context
        .get_storage()
        .full_migrate(workspace_id.clone(), None, false)
        .await;
    info!(
        "{} stop collaborate with workspace {}",
        identifier, workspace_id
    );
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;
    use jwst::{JwstResult, Workspace};
    use jwst_storage::JwstStorage;
    use nanoid::nanoid;
    use tokio::sync::RwLock;
    use yrs::{updates::decoder::Decode, Doc, ReadTxn, StateVector, Transact, Update};

    struct ServerContext {
        channel: BroadcastChannels,
        storage: JwstStorage,
    }

    impl ServerContext {
        pub async fn new() -> Arc<Self> {
            let storage = JwstStorage::new("postgresql://affine:affine@localhost:5432/affine")
                .await
                .unwrap();

            Arc::new(Self {
                channel: RwLock::new(HashMap::new()),
                storage,
            })
        }
    }

    impl RpcContextImpl<'_> for ServerContext {
        fn get_storage(&self) -> &JwstStorage {
            &self.storage
        }

        fn get_channel(&self) -> &BroadcastChannels {
            &self.channel
        }
    }

    async fn create_broadcasting_workspace(
        init_state: &[u8],
        server: Arc<ServerContext>,
        id: &str,
    ) -> (Workspace, Sender<Message>) {
        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(Update::decode_v1(init_state).unwrap());
        let workspace = Workspace::from_doc(doc, id);

        let doc = workspace.doc();

        let (tx, rx) = memory_connector(doc, rand::random::<usize>());
        {
            let tx = tx.clone();
            tokio::spawn(handle_connector(
                server,
                "test".into(),
                nanoid!(),
                move || (tx, rx),
            ));
        }

        (workspace, tx)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 64)]
    async fn sync_test() -> JwstResult<()> {
        jwst_logger::init_logger();
        let server = ServerContext::new().await;
        let ws = server.get_workspace("test").await.unwrap();

        let init_state = ws
            .doc()
            .transact()
            .encode_state_as_update_v1(&StateVector::default());

        let (doc1, doc1_tx) =
            create_broadcasting_workspace(&init_state, server.clone(), "test").await;
        let (doc2, doc2_tx) =
            create_broadcasting_workspace(&init_state, server.clone(), "test").await;

        doc1.with_trx(|mut t| {
            let space = t.get_space("space");
            let block1 = space.create(&mut t.trx, "block1", "flavor1");
            block1.set(&mut t.trx, "key1", "val1");
        });

        // await the task to make sure the doc1 is broadcasted before check doc2
        sleep(Duration::from_millis(1)).await;

        doc2.with_trx(|mut t| {
            let space = t.get_space("space");
            let block1 = space.get(&mut t.trx, "block1").unwrap();

            assert_eq!(block1.flavor(&t.trx), "flavor1");
            assert_eq!(block1.get(&t.trx, "key1").unwrap().to_string(), "val1");
        });

        ws.with_trx(|mut t| {
            let space = t.get_space("space");
            let block1 = space.get(&mut t.trx, "block1").unwrap();

            assert_eq!(block1.flavor(&t.trx), "flavor1");
            assert_eq!(block1.get(&t.trx, "key1").unwrap().to_string(), "val1");
        });

        doc1_tx.send(Message::Close).await.unwrap();
        doc2_tx.send(Message::Close).await.unwrap();

        Ok(())
    }
}
