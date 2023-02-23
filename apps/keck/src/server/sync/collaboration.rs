use super::*;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Response,
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use jwst::{sync_encode_update, Workspace};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::channel;

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub async fn auth_handler(Path(workspace_id): Path<String>) -> Json<WebSocketAuthentication> {
    info!("auth: {}", workspace_id);
    Json(WebSocketAuthentication {
        protocol: "AFFiNE".to_owned(),
    })
}

pub async fn upgrade_handler(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(["AFFiNE"])
        .on_upgrade(|socket| async move { handle_socket(socket, workspace, context.clone()).await })
}

fn subscribe_handler(
    context: Arc<Context>,
    workspace: &mut Workspace,
    uuid: String,
    ws_id: String,
) {
    if let Some(sub) = workspace.observe(move |_, e| {
        debug!("workspace changed: {}, {:?}", ws_id, &e.update);
        let update = sync_encode_update(&e.update);

        let context = context.clone();
        let uuid = uuid.clone();
        let ws_id = ws_id.clone();
        tokio::spawn(async move {
            let mut closed = vec![];

            for item in context.channel.iter() {
                let ((ws, id), tx) = item.pair();
                debug!("workspace send: {}, {}", ws_id, id);
                if &ws_id == ws && id != &uuid {
                    if tx.is_closed() {
                        debug!("workspace closed: {}, {}", ws_id, id);
                        closed.push(id.clone());
                    } else if let Err(e) = tx.send(Message::Binary(update.clone())).await {
                        if !tx.is_closed() {
                            error!("on observe_update error: {}", e);
                        }
                    }
                }
            }
            for id in closed {
                context.channel.remove(&(ws_id.clone(), id));
            }
        });
    }) {
        std::mem::forget(sub);
    } else {
        error!("on observe error");
    }
}

async fn handle_socket(socket: WebSocket, workspace_id: String, context: Arc<Context>) {
    info!("collaboration: {}", workspace_id);

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
        tokio::spawn(async move {
            use tokio::time::{sleep, Duration};
            loop {
                sleep(Duration::from_secs(10)).await;

                if let Ok(workspace) = context.storage.get_workspace(&workspace_id).await {
                    let update = workspace.read().await.sync_migration();
                    if let Err(e) = context
                        .storage
                        .docs()
                        .full_migrate(&workspace_id, update)
                        .await
                    {
                        error!("db write error: {}", e.to_string());
                    }
                } else {
                    break;
                }
            }
        });
    }

    let uuid = Uuid::new_v4().to_string();
    context
        .channel
        .insert((workspace_id.clone(), uuid.clone()), tx.clone());

    if let Ok(init_data) = {
        let ws = match context.storage.create_workspace(&workspace_id).await {
            Ok(doc) => doc,
            Err(e) => {
                error!("Failed to init doc: {}", e);
                return;
            }
        };

        let mut ws = ws.write().await;

        subscribe_handler(context.clone(), &mut ws, uuid.clone(), workspace_id.clone());

        ws.sync_init_message()
    } {
        if tx.send(Message::Binary(init_data)).await.is_err() {
            context.channel.remove(&(workspace_id, uuid));
            // client disconnected
            return;
        }
    } else {
        context.channel.remove(&(workspace_id, uuid));
        // client disconnected
        return;
    }

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Binary(binary)) = msg {
            let payload = {
                let workspace = context
                    .storage
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

    context.channel.remove(&(workspace_id, uuid));
}
