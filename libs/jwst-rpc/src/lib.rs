mod broadcast;
mod client;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use client::start_client;

use axum::{
    extract::ws::{Message, WebSocket},
    Error,
};
use broadcast::subscribe;
use futures::{sink::SinkExt, stream::StreamExt};
use jwst::{debug, error, info, trace, warn};
use jwst_storage::JwstStorage;
use lru_time_cache::LruCache;
use std::{collections::hash_map::Entry, sync::Arc};
use tokio::{
    sync::broadcast::channel as broadcast,
    time::{sleep, Duration},
};
use tokio_tungstenite::tungstenite::Error as SocketError;

pub trait ContextImpl<'a> {
    fn get_storage(&self) -> &JwstStorage;
    fn get_channel(&self) -> &BroadcastChannels;
}

#[inline]
fn is_connection_closed(error: Error) -> bool {
    if let Ok(e) = error.into_inner().downcast::<SocketError>() {
        matches!(e.as_ref(), SocketError::ConnectionClosed)
    } else {
        false
    }
}

pub async fn handle_socket(
    socket: WebSocket,
    workspace_id: String,
    context: Arc<impl ContextImpl<'static> + Send + Sync + 'static>,
    identifier: String,
) {
    info!("{} collaborate with workspace {}", identifier, workspace_id);

    let (mut socket_tx, mut socket_rx) = socket.split();
    let (tx, mut rx) = match context
        .get_channel()
        .write()
        .await
        .entry(workspace_id.clone())
    {
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

    let mut server_update = match context
        .get_storage()
        .docs()
        .remote()
        .write()
        .await
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

        let _sub = subscribe(&mut ws, tx);
        // just keep the ownership
        std::mem::forget(_sub);

        ws.sync_init_message()
    } {
        if socket_tx.send(Message::Binary(init_data)).await.is_err() {
            // client disconnected
            if let Err(e) = socket_tx.send(Message::Close(None)).await {
                error!("failed to send close event: {}", e);
            }
            return;
        }
    } else {
        if let Err(e) = socket_tx.send(Message::Close(None)).await {
            error!("failed to send close event: {}", e);
        }
        return;
    }

    let mut dedup_cache = LruCache::with_expiry_duration_and_capacity(Duration::from_secs(5), 128);

    loop {
        tokio::select! {
            Some(msg) = socket_rx.next() => {
                if let Ok(Message::Binary(binary)) = msg {
                    debug!("recv from remote: {}bytes", binary.len());

                    let mut workspace = context
                        .get_storage()
                        .get_workspace(&workspace_id)
                        .await
                        .expect("workspace not found");

                    let message = workspace.sync_decode_message(&binary);

                    for reply in message {
                        if !dedup_cache.contains_key(&reply) {
                            debug!("send pipeline message by {identifier:?}: {}", reply.len());
                            if let Err(e) = socket_tx.send(Message::Binary(reply.clone())).await {
                                let error = e.to_string();
                                if is_connection_closed(e) {
                                    return;
                                } else {
                                    error!("socket send error: {}", error);
                                }
                            }
                            dedup_cache.insert(reply, ());
                        }
                    }
                }
            },
            Ok(msg) = server_update.recv()=> {
                if !dedup_cache.contains_key(&msg) {
                    debug!("recv from server update: {:?}", msg);
                    if let Err(e) = socket_tx.send(Message::Binary(msg.clone())).await {
                        error!("send error: {}", e);
                        break;
                    }
                    dedup_cache.insert(msg, ());
                }
            },
            Ok(msg) = rx.recv()=> {
                if let BroadcastType::Broadcast(msg) = msg {
                    if !dedup_cache.contains_key(&msg) {
                        debug!("recv from broadcast update: {:?}bytes", msg.len());
                        if let Err(e) = socket_tx.send(Message::Binary(msg.clone())).await {
                            error!("send error: {}", e);
                            break;
                        }
                        dedup_cache.insert(msg, ());
                    }
                } else if matches!(&msg, BroadcastType::CloseUser(user) if user == &identifier) || matches!(msg, BroadcastType::CloseAll) {
                    if let Err(e) = socket_tx.send(Message::Close(None)).await {
                        error!("failed to send close event: {}", e);
                    }
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
}
