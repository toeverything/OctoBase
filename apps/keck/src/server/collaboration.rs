use super::*;
use crate::sync::{decode_remote_message, encode_init_update, encode_update};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Response,
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use jwst::Workspace;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use yrs::StateVector;

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub fn collaboration_handler(router: Router) -> Router {
    router.nest_service(
        "/collaboration/:workspace",
        post(collaboration::auth_handler).get(collaboration::upgrade_handler),
    )
}

async fn auth_handler(Path(workspace): Path<String>) -> Json<WebSocketAuthentication> {
    info!("auth: {}", workspace);
    Json(WebSocketAuthentication {
        protocol: "AFFiNE".to_owned(),
    })
}

async fn upgrade_handler(
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
    let sub = workspace.observe(move |_, e| {
        let update = encode_update(&e.update);

        let context = context.clone();
        let uuid = uuid.clone();
        let ws_id = ws_id.clone();
        tokio::spawn(async move {
            let mut closed = vec![];

            for item in context.channel.iter() {
                let ((ws, id), tx) = item.pair();
                if &ws_id == ws && id != &uuid {
                    if tx.is_closed() {
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
    });
    std::mem::forget(sub);
}

async fn handle_socket(socket: WebSocket, workspace: String, context: Arc<Context>) {
    info!("collaboration: {}", workspace);

    let (mut socket_tx, mut socket_rx) = socket.split();
    let (tx, mut rx) = channel(100);

    {
        // socket thread
        let workspace = workspace.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = socket_tx.send(msg).await {
                    error!("send error: {}", e);
                    break;
                }
            }
            info!("socket final: {}", workspace);
        });
    }

    {
        let workspace = workspace.clone();
        let context = context.clone();
        tokio::spawn(async move {
            use tokio::time::{sleep, Duration};
            loop {
                sleep(Duration::from_secs(10)).await;

                let update = {
                    if let Some(workspace) = context.workspace.get(&workspace) {
                        Some(
                            workspace
                                .lock()
                                .await
                                .doc()
                                .encode_state_as_update_v1(&StateVector::default()),
                        )
                    } else {
                        None
                    }
                };

                if let Some(update) = update {
                    if let Err(e) = context.db.full_migrate(&workspace, update).await {
                        error!("db write error: {}", e.to_string());
                    }
                }
            }
        });
    }

    let uuid = Uuid::new_v4().to_string();
    context
        .channel
        .insert((workspace.clone(), uuid.clone()), tx.clone());

    let init_data = {
        let ws = match init_workspace(&context, &workspace).await {
            Ok(doc) => doc,
            Err(e) => {
                error!("Failed to init doc: {}", e);
                return;
            }
        };

        let mut ws = ws.lock().await;

        subscribe_handler(context.clone(), &mut ws, uuid.clone(), workspace.clone());

        encode_init_update(ws.doc())
    };

    if tx.send(Message::Binary(init_data)).await.is_err() {
        context.channel.remove(&(workspace, uuid));
        // client disconnected
        return;
    }

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Binary(binary)) = msg {
            let payload = {
                let workspace = context.workspace.get(&workspace).unwrap();
                let workspace = workspace.value().lock().await;
                let doc = workspace.doc();

                use std::panic::{catch_unwind, AssertUnwindSafe};
                catch_unwind(AssertUnwindSafe(|| decode_remote_message(doc, binary)))
            };
            if let Ok((binary, update)) = payload {
                if let Some(update) = update {
                    if let Err(e) = context.db.update(&workspace, update).await {
                        error!("db write error: {}", e.to_string());
                    }
                }
                if let Some(binary) = binary {
                    if let Err(e) = tx.send(Message::Binary(binary)).await {
                        if !tx.is_closed() {
                            error!("socket send error: {}", e.to_string());
                        }
                        // client disconnected
                        return;
                    }
                }
            }
        }
    }

    context.channel.remove(&(workspace, uuid));
}
