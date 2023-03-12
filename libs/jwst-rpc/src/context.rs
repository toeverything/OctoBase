use super::{
    broadcast::{subscribe, BroadcastChannels, BroadcastType},
    *,
};
use async_trait::async_trait;
use axum::extract::ws::Message;
use jwst::{JwstResult, Workspace};
use jwst_storage::JwstStorage;
use tokio::sync::{
    broadcast::{channel as broadcast, Receiver as BroadcastReceiver},
    mpsc::{Receiver as MpscReceiver, Sender as MpscSender},
};

#[async_trait]
pub trait RpcContextImpl<'a> {
    fn get_storage(&self) -> &JwstStorage;
    fn get_channel(&self) -> &BroadcastChannels;

    async fn get_workspace(&self, id: &str) -> JwstResult<Workspace> {
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

    async fn join_broadcast(&self, workspace: &mut Workspace) -> BroadcastReceiver<BroadcastType> {
        // broadcast channel
        let (broadcast_tx, broadcast_rx) =
            match self.get_channel().write().await.entry(workspace.id()) {
                Entry::Occupied(tx) => {
                    let sender = tx.get();
                    (sender.clone(), sender.subscribe())
                }
                Entry::Vacant(v) => {
                    let (tx, rx) = broadcast(100);
                    v.insert(tx.clone());
                    (tx, rx)
                }
            };

        subscribe(workspace, broadcast_tx).await;

        broadcast_rx
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
