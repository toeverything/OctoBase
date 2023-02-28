mod broadcast;
mod channel;
mod client;

pub use channel::Channels;
pub use client::start_client;

use axum::extract::ws::{Message, WebSocket};
use broadcast::subscribe;
use channel::ChannelItem;
use dashmap::mapref::entry::Entry;
use futures::{sink::SinkExt, stream::StreamExt};
use jwst::{debug, error, info, trace, warn};
use jwst_storage::JwstStorage;
use std::sync::Arc;
use tokio::{
    sync::broadcast::channel as broadcast,
    sync::mpsc::channel,
    time::{sleep, Duration},
};

pub trait ContextImpl<'a> {
    fn get_storage(&self) -> &JwstStorage;
    fn get_channel(&self) -> &Channels;
}

pub async fn handle_socket(
    socket: WebSocket,
    workspace_id: String,
    context: Arc<impl ContextImpl<'static> + Send + Sync + 'static>,
    identifier: String,
) {
    info!("{} collaborate with workspace {}", identifier, workspace_id);

    let (mut socket_tx, mut socket_rx) = socket.split();
    let (tx, mut rx) = channel(100);

    let channel_item = ChannelItem::new(&workspace_id, &identifier);
    context
        .get_channel()
        .insert(channel_item.clone(), tx.clone());
    debug!("{workspace_id} add channel: {identifier}");

    let mut server_update = match context
        .get_storage()
        .docs()
        .remote()
        .entry(workspace_id.clone())
    {
        Entry::Occupied(tx) => tx.get().subscribe(),
        Entry::Vacant(v) => {
            let (tx, rx) = broadcast(100);
            v.insert(tx);
            rx
        }
    };

    if let Ok(init_data) = {
        let mut ws = context
            .get_storage()
            .create_workspace(&workspace_id)
            .await
            .expect("create workspace failed, please check if the workspace_id is valid or not");

        let sub = subscribe(context.clone(), &mut ws, &channel_item);
        std::mem::forget(sub);

        ws.sync_init_message()
    } {
        if tx.send(Some(init_data)).await.is_err() {
            context.get_channel().remove(&channel_item);
            debug!("{workspace_id} remove channel: {identifier}");
            // client disconnected
            return;
        }
    } else {
        context.get_channel().remove(&channel_item);
        // client disconnected
        return;
    }

    loop {
        tokio::select! {
            Some(msg) = socket_rx.next() => {
                let mut success = true;
                if let Ok(Message::Binary(binary)) = msg {
                    debug!("recv from remote: {}bytes", binary.len());
                    let payload = {
                        let mut workspace = context
                            .get_storage()
                            .get_workspace(&workspace_id)
                            .await
                            .expect("workspace not found");

                        use std::panic::{catch_unwind, AssertUnwindSafe};
                        catch_unwind(AssertUnwindSafe(|| workspace.sync_decode_message(&binary)))
                    };
                    if let Ok(messages) = payload {
                        for reply in messages {
                            debug!("send pipeline message by {identifier:?}");
                            if let Err(e) = tx.send(Some(reply)).await {
                                if !tx.is_closed() {
                                    error!("socket send error: {}", e.to_string());
                                } else {
                                    // client disconnected
                                    success = false;
                                    break;
                                }
                            }
                        }
                    }
                }
                if !success {
                    break
                }
            },
            Ok(msg) = server_update.recv() => {
                debug!("recv from server update: {:?}", msg);
                if let Err(e) = socket_tx.send(Message::Binary(msg)).await {
                    error!("send error: {}", e);
                    break;
                }
            },
            Some(msg) = rx.recv() => {
                trace!(
                    "recv from channel: {}bytes",
                    msg.as_ref().map(|v| v.len() as isize).unwrap_or(-1)
                );
                if let Err(e) = socket_tx
                    .send(msg.map(Message::Binary).unwrap_or(Message::Close(None)))
                    .await
                {
                    error!("send error: {}", e);
                    break;
                }
            },
            _ = sleep(Duration::from_secs(5)) => {
                context
                    .get_storage()
                    .full_migrate(workspace_id.clone(), None, false)
                    .await;
            }
        }
    }

    context.get_channel().remove(&channel_item);
}
