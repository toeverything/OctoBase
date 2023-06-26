use std::collections::HashMap;

use super::{
    broadcast::{subscribe, BroadcastChannels, BroadcastType},
    *,
};
use async_trait::async_trait;
use jwst::{DocStorage, Workspace};
use jwst_codec::{CrdtReader, RawDecoder};
use jwst_storage::{JwstStorage, JwstStorageResult};
use tokio::sync::{
    broadcast::{channel as broadcast, Receiver as BroadcastReceiver, Sender as BroadcastSender},
    mpsc::{Receiver as MpscReceiver, Sender as MpscSender},
    Mutex,
};

use yrs::merge_updates_v1;

fn merge_updates(id: &str, updates: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    match merge_updates_v1(
        &updates
            .iter()
            .map(std::ops::Deref::deref)
            .collect::<Vec<_>>(),
    ) {
        Ok(update) => {
            info!("merge {} updates", updates.len());
            vec![update]
        }
        Err(e) => {
            error!("failed to merge update of {}: {}", id, e);
            updates
        }
    }
}
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
    ) -> BroadcastSender<BroadcastType> {
        let id = workspace.id();
        info!("join_broadcast, {:?}", workspace.id());
        // broadcast channel
        let broadcast_tx = match self.get_channel().write().await.entry(id.clone()) {
            Entry::Occupied(tx) => tx.get().clone(),
            Entry::Vacant(v) => {
                let (tx, _) = broadcast(100);
                v.insert(tx.clone());
                tx.clone()
            }
        };

        // Listen to changes of the local workspace, encode changes in awareness and Doc, and broadcast them.
        // It returns the 'broadcast_rx' object to receive the content that was sent
        subscribe(workspace, identifier.clone(), broadcast_tx.clone()).await;

        // save update thread
        self.save_update(&id, identifier, broadcast_tx.subscribe())
            .await;

        // returns the 'broadcast_tx' which can be subscribed later, to receive local workspace changes
        broadcast_tx
    }

    async fn save_update(
        &self,
        id: &str,
        identifier: String,
        mut broadcast: BroadcastReceiver<BroadcastType>,
    ) {
        let docs = self.get_storage().docs().clone();
        let id = id.to_string();

        tokio::spawn(async move {
            let updates = Arc::new(Mutex::new(HashMap::<String, Vec<Vec<u8>>>::new()));

            let handler = {
                let id = id.clone();
                let updates = updates.clone();
                tokio::spawn(async move {
                    while let Ok(data) = broadcast.recv().await {
                        match data {
                            BroadcastType::BroadcastRawContent(update) => {
                                trace!("receive update: {}", update.len());
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
                        }
                    }
                    debug!("save update thread {id}-{identifier} closed");
                })
            };

            loop {
                {
                    let mut updates = updates.lock().await;
                    if !updates.is_empty() {
                        for (guid, updates) in updates.drain() {
                            debug!("save {} updates from {guid}", updates.len());

                            let updates = merge_updates(&id, updates);

                            for update in updates {
                                if let Err(e) =
                                    docs.update_doc(id.clone(), guid.clone(), &update).await
                                {
                                    error!("failed to save update of {}: {:?}", id, e);
                                }
                            }
                        }
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
    ) {
        // collect messages from remote
        let identifier = identifier.to_owned();
        let mut workspace = self
            .get_storage()
            .get_workspace(&id)
            .await
            .expect("workspace not found");
        tokio::spawn(async move {
            while let Some(binary) = remote_rx.recv().await {
                if binary == [0, 2, 2, 0, 0] {
                    // skip empty update
                    continue;
                }
                trace!("apply_change: recv binary: {:?}", binary);
                let ts = Instant::now();
                let message = workspace.sync_decode_message(&binary).await;
                if ts.elapsed().as_micros() > 50 {
                    debug!("apply remote update cost: {}ms", ts.elapsed().as_micros());
                }

                for reply in message {
                    trace!("send pipeline message by {identifier:?}: {}", reply.len());
                    if local_tx.send(Message::Binary(reply.clone())).await.is_err() {
                        // pipeline was closed
                        break;
                    }
                }
            }
        });
    }
}
