use std::collections::HashMap;

use super::{
    broadcast::{subscribe, BroadcastChannels, BroadcastType},
    *,
};
use async_trait::async_trait;
use chrono::Utc;
use jwst_codec::{CrdtReader, RawDecoder};
use jwst_core::{DocStorage, Workspace};
use jwst_core_storage::{JwstStorage, JwstStorageResult};
use tokio::sync::{
    broadcast::{
        channel as broadcast, error::RecvError, Receiver as BroadcastReceiver,
        Sender as BroadcastSender,
    },
    mpsc::{Receiver as MpscReceiver, Sender as MpscSender},
    Mutex,
};

#[async_trait]
pub trait RpcContextImpl<'a> {
    fn get_storage(&self) -> &JwstStorage;
    fn get_channel(&self) -> &BroadcastChannels;

    async fn get_workspace(&self, id: &str) -> JwstStorageResult<Workspace> {
        self.get_storage().create_workspace(id).await
    }

    async fn join_server_broadcast(&self, id: &str) -> BroadcastReceiver<Vec<u8>> {
        let id = id.into();
        match self.get_storage().docs().remote().write().await.entry(id) {
            Entry::Occupied(tx) => tx.get().subscribe(),
            Entry::Vacant(v) => {
                let (tx, rx) = broadcast(100);
                v.insert(tx);
                rx
            }
        }
    }

    async fn join_broadcast(
        &self,
        workspace: &mut Workspace,
        identifier: String,
        last_synced: Sender<i64>,
    ) -> BroadcastSender<BroadcastType> {
        let id = workspace.id();
        info!("join_broadcast, {:?}", workspace.id());
        // broadcast channel
        let broadcast_tx = match self.get_channel().write().await.entry(id.clone()) {
            Entry::Occupied(tx) => tx.get().clone(),
            Entry::Vacant(v) => {
                let (tx, _) = broadcast(10240);
                v.insert(tx.clone());
                tx.clone()
            }
        };

        // Listen to changes of the local workspace, encode changes in awareness and Doc, and broadcast them.
        // It returns the 'broadcast_rx' object to receive the content that was sent
        subscribe(workspace, identifier.clone(), broadcast_tx.clone()).await;

        // save update thread
        self.save_update(&id, identifier, broadcast_tx.subscribe(), last_synced)
            .await;

        // returns the 'broadcast_tx' which can be subscribed later, to receive local workspace changes
        broadcast_tx
    }

    async fn save_update(
        &self,
        id: &str,
        identifier: String,
        mut broadcast: BroadcastReceiver<BroadcastType>,
        last_synced: Sender<i64>,
    ) {
        let docs = self.get_storage().docs().clone();
        let id = id.to_string();

        tokio::spawn(async move {
            trace!("save update thread {id}-{identifier} started");
            let updates = Arc::new(Mutex::new(HashMap::<String, Vec<Vec<u8>>>::new()));

            let handler = {
                let id = id.clone();
                let updates = updates.clone();
                tokio::spawn(async move {
                    loop {
                        match broadcast.recv().await {
                            Ok(data) => match data {
                                BroadcastType::BroadcastRawContent(update) => {
                                    trace!("receive raw update: {}", update.len());
                                    let mut decoder = RawDecoder::new(update);
                                    if let Ok(guid) = decoder.read_var_string() {
                                        match updates.lock().await.entry(guid) {
                                            Entry::Occupied(mut updates) => {
                                                updates.get_mut().push(decoder.drain());
                                            }
                                            Entry::Vacant(v) => {
                                                v.insert(vec![decoder.drain()]);
                                            }
                                        };
                                    };
                                }
                                BroadcastType::CloseUser(user) if user == identifier => break,
                                BroadcastType::CloseAll => break,
                                _ => {}
                            },
                            Err(RecvError::Lagged(num)) => {
                                debug!("save update thread {id}-{identifier} lagged: {num}");
                            }
                            Err(RecvError::Closed) => {
                                debug!("save update thread {id}-{identifier} closed");
                                break;
                            }
                        }
                    }
                })
            };

            loop {
                {
                    let mut updates = updates.lock().await;
                    if !updates.is_empty() {
                        for (guid, updates) in updates.drain() {
                            debug!("save {} updates from {guid}", updates.len());

                            for update in updates {
                                if let Err(e) =
                                    docs.update_doc(id.clone(), guid.clone(), &update).await
                                {
                                    error!("failed to save update of {}: {:?}", id, e);
                                }
                            }
                        }
                        last_synced
                            .send(Utc::now().timestamp_millis())
                            .await
                            .unwrap();
                    } else if handler.is_finished() {
                        break;
                    }
                }
                sleep(Duration::from_secs(1)).await;
            }
        });
    }

    async fn apply_change(
        &self,
        id: &str,
        identifier: &str,
        local_tx: MpscSender<Message>,
        mut remote_rx: MpscReceiver<Vec<u8>>,
        last_synced: Sender<i64>,
    ) {
        // collect messages from remote
        let identifier = identifier.to_owned();
        let id = id.to_string();
        let mut workspace = self
            .get_storage()
            .get_workspace(&id)
            .await
            .expect("workspace not found");
        tokio::spawn(async move {
            trace!("apply update thread {id}-{identifier} started");
            let mut updates = Vec::<Vec<u8>>::new();

            loop {
                tokio::select! {
                    binary = remote_rx.recv() => {
                        if let Some(binary) = binary {
                            if binary == [0, 2, 2, 0, 0] || binary == [1, 1, 0] {
                                // skip empty update
                                continue;
                            }
                            trace!("apply_change: recv binary: {:?}", binary.len());
                            updates.push(binary);
                        } else {
                            break;
                        }
                    },
                     _ = sleep(Duration::from_millis(100)) => {
                        if !updates.is_empty() {
                            debug!("apply {} updates for {id}", updates.len());

                            let updates = updates.drain(..).collect::<Vec<_>>();
                            let updates_len = updates.len();
                            let ts = Instant::now();
                            let message = workspace.sync_messages(updates).await;
                            if ts.elapsed().as_micros() > 50 {
                                debug!(
                                    "apply {updates_len} remote update cost: {}ms",
                                    ts.elapsed().as_micros(),
                                );
                            }

                            for reply in message {
                                trace!("send pipeline message by {identifier:?}: {}", reply.len());
                                if local_tx.send(Message::Binary(reply.clone())).await.is_err() {
                                    // pipeline was closed
                                    break;
                                }
                            }

                            last_synced
                                .send(Utc::now().timestamp_millis())
                                .await
                                .unwrap();
                        }
                     }
                }
            }
        });
    }
}
