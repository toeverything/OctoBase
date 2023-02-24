mod broadcast;
mod channel;

pub use channel::Channels;

use axum::extract::ws::{Message, WebSocket};
use broadcast::subscribe;
use channel::ChannelItem;
use futures::{sink::SinkExt, stream::StreamExt};
use jwst::{error, info, trace};
use jwst_storage::JwstStorage;
use std::sync::Arc;
use tokio::sync::mpsc::channel;

pub trait ContextImpl<'a> {
    fn get_storage(&self) -> JwstStorage;
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

    {
        // socket thread
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = socket_tx.send(msg).await {
                    error!("send error: {}", e);
                    break;
                }
            }
            info!("socket final: {}", workspace_id);
        });
    }

    {
        let workspace_id = workspace_id.clone();
        let context = context.clone();
        let storage = context.get_storage();
        tokio::spawn(async move {
            use tokio::time::{sleep, Duration};
            loop {
                sleep(Duration::from_secs(10)).await;

                storage.full_migrate(workspace_id.clone(), None).await;
            }
        });
    }

    let channel = ChannelItem::new(&workspace_id, &identifier);

    context.get_channel().insert(channel.clone(), tx.clone());

    if let Ok(init_data) = {
        let ws = context
            .get_storage()
            .create_workspace(&workspace_id)
            .await
            .expect("create workspace failed, please check if the workspace_id is valid or not");

        let mut ws = ws.write().await;

        let sub = subscribe(context.clone(), &mut ws, &channel);
        std::mem::forget(sub);

        ws.sync_init_message()
    } {
        if tx.send(Message::Binary(init_data)).await.is_err() {
            context.get_channel().remove(&channel);
            // client disconnected
            return;
        }
    } else {
        context.get_channel().remove(&channel);
        // client disconnected
        return;
    }

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Binary(binary)) = msg {
            let payload = {
                let workspace = context
                    .get_storage()
                    .get_workspace(&workspace_id)
                    .await
                    .expect("workspace not found");
                let mut workspace = workspace.write().await;

                use std::panic::{catch_unwind, AssertUnwindSafe};
                catch_unwind(AssertUnwindSafe(|| workspace.sync_decode_message(&binary)))
            };
            if let Ok(messages) = payload {
                for reply in messages {
                    if let Err(e) = tx.send(Message::Binary(reply)).await {
                        if !tx.is_closed() {
                            error!("socket send error: {}", e.to_string());
                        } else {
                            // client disconnected
                            return;
                        }
                    }
                }
            }
        }
    }

    context.get_channel().remove(&channel);
}
