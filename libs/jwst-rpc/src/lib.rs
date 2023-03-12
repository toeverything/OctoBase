#![feature(mutex_unpoison, type_alias_impl_trait)]

mod broadcast;
mod client;
mod connector;
mod context;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use client::start_client;
pub use connector::socket_connector;
pub use context::RpcContextImpl;

use jwst::{debug, error, info, trace, warn};
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};

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
