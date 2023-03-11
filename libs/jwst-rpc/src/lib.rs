#![feature(mutex_unpoison)]

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
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::{broadcast::channel as broadcast, mpsc::channel},
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

    // send to remote pipeline
    let (pipeline_tx, mut pipeline_rx) = channel(100);
    {
        // socket thread
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move {
            while let Some(msg) = pipeline_rx.recv().await {
                if let Err(e) = socket_tx.send(msg).await {
                    let error = e.to_string();
                    if is_connection_closed(e) {
                        break;
                    } else {
                        error!("socket send error: {}", error);
                    }
                }
            }
            info!("socket final: {}", workspace_id);
        });
    }

    // broadcast channel
    let (broadcast_tx, mut broadcast_rx) = match context
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

        let _sub = subscribe(&mut ws, broadcast_tx);
        // just keep the ownership
        std::mem::forget(_sub);

        ws.sync_init_message()
    } {
        if pipeline_tx.send(Message::Binary(init_data)).await.is_err() {
            // client disconnected
            if let Err(e) = pipeline_tx.send(Message::Close(None)).await {
                error!("failed to send close event: {}", e);
            }
            return;
        }
    } else {
        if let Err(e) = pipeline_tx.send(Message::Close(None)).await {
            error!("failed to send close event: {}", e);
        }
        return;
    }

    'sync: loop {
        tokio::select! {
            Some(msg) = socket_rx.next() => {
                let ts = Instant::now();
                if let Ok(Message::Binary(binary)) = msg {
                    trace!("recv from remote: {}bytes", binary.len());

                    let ts = Instant::now();
                    let mut workspace = context
                        .get_storage()
                        .get_workspace(&workspace_id)
                        .await
                        .expect("workspace not found");

                    // TODO: apply is very slow, need to optimize
                    let message = workspace.sync_decode_message(&binary);
                    if ts.elapsed().as_micros() > 100 {
                        debug!("apply remote update cost: {}ms", ts.elapsed().as_micros());
                    }

                    let ts = Instant::now();

                    for reply in message {
                        trace!("send pipeline message by {identifier:?}: {}", reply.len());
                        if pipeline_tx
                            .send(Message::Binary(reply.clone()))
                            .await
                            .is_err()
                        {
                            // pipeline was closed
                            break 'sync;
                        }
                    }

                    if ts.elapsed().as_micros() > 100 {
                        debug!("send remote update cost: {}ms", ts.elapsed().as_micros());
                    }
                }

                if ts.elapsed().as_micros() > 100 {
                    debug!("process remote update cost: {}ms", ts.elapsed().as_micros());
                }
            },
            Ok(msg) = server_update.recv()=> {
                let ts = Instant::now();
                debug!("recv from server update: {:?}", msg);
                if pipeline_tx
                    .send(Message::Binary(msg.clone()))
                    .await
                    .is_err()
                {
                    // pipeline was closed
                    break 'sync;
                }
                if ts.elapsed().as_micros() > 100 {
                    debug!("process server update cost: {}ms", ts.elapsed().as_micros());
                }

            },
            Ok(msg) = broadcast_rx.recv()=> {
                let ts = Instant::now();
                match msg {
                    BroadcastType::BroadcastAwareness(data) => {
                        let ts = Instant::now();
                        trace!(
                            "recv awareness update from broadcast: {:?}bytes",
                            data.len()
                        );
                        if pipeline_tx
                            .send(Message::Binary(data.clone()))
                            .await
                            .is_err()
                        {
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
                        if pipeline_tx
                            .send(Message::Binary(data.clone()))
                            .await
                            .is_err()
                        {
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
                        if pipeline_tx.send(Message::Close(None)).await.is_err() {
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
                        if pipeline_tx.send(Message::Close(None)).await.is_err() {
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
                if pipeline_tx.is_closed() || pipeline_tx.send(Message::Ping(vec![])).await.is_err() {
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
