use super::*;
use axum::{
    extract::{
        ws::{close_code, CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Response,
};
use base64::Engine;
use futures::{sink::SinkExt, stream::StreamExt};
use jwst::{sync_encode_update, Workspace};
use jwst_logger::error;
use jwst_logger::info;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use uuid::Uuid;
use y_sync::sync::Message as YMessage;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub fn make_ws_route() -> Router {
    Router::new().route("/:id", get(ws_handler))
}

#[derive(Deserialize)]
struct Param {
    token: String,
}

async fn ws_handler(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    Query(Param { token }): Query<Param>,
    ws: WebSocketUpgrade,
) -> Response {
    let user: Option<RefreshToken> = URL_SAFE_ENGINE
        .decode(token)
        .ok()
        .and_then(|byte| match ctx.decrypt_aes(byte) {
            Ok(data) => data,
            Err(_) => None,
        })
        .and_then(|data| serde_json::from_slice(&data).ok());

    let user = if let Some(user) = user {
        if let Ok(true) = ctx.db.verify_refresh_token(&user).await {
            Some(user.user_id)
        } else {
            None
        }
    } else {
        None
    };

    ws.protocols(["AFFiNE"])
        .on_upgrade(move |socket| async move {
            handle_socket(socket, workspace, ctx.clone(), user).await
        })
}

fn subscribe_handler(
    context: Arc<Context>,
    workspace: &mut Workspace,
    uuid: String,
    ws_id: String,
) {
    let sub = workspace.observe(move |_, e| {
        let update = sync_encode_update(&e.update);

        let context = context.clone();
        let uuid = uuid.clone();
        let ws_id = ws_id.clone();
        tokio::spawn(async move {
            let mut closed = vec![];

            for item in context.channel.iter() {
                let ((ws, user_id, id), tx) = item.pair();
                if &ws_id == ws && id != &uuid {
                    if tx.is_closed() {
                        closed.push((user_id.clone(), id.clone()));
                    } else if let Err(e) = tx.send(Message::Binary(update.clone())).await {
                        if !tx.is_closed() {
                            error!("on observe_update error: {}", e);
                        }
                    }
                }
            }
            for close in closed {
                let (user_id, id) = close;
                context.channel.remove(&(ws_id.clone(), user_id, id));
            }
        });
    });
    std::mem::forget(sub);
}

fn subscribe_metadata_handler(context: Arc<Context>, workspace: &mut Workspace, ws_id: String) {
    let sub_metadata = workspace.observe_metadata(move |_, _e| {
        context
            .user_channel
            .update_workspace(ws_id.clone(), context.clone());
    });
    std::mem::forget(sub_metadata);
}

fn subscribe_awareness_handler(
    context: Arc<Context>,
    workspace: &mut Workspace,
    uuid: String,
    ws_id: String,
) {
    let awareness_sub = workspace.on_awareness_update(move |awareness, e| {
        let update_result =
            awareness.update_with_clients([e.added(), e.updated(), e.removed()].concat());
        if let Ok(aw_update) = update_result {
            let mut encoder = EncoderV1::new();
            YMessage::Awareness(aw_update).encode(&mut encoder);
            let update = encoder.to_vec();
            let context = context.clone();
            let uuid = uuid.clone();
            let ws_id = ws_id.clone();
            tokio::spawn(async move {
                let mut closed = vec![];

                for item in context.channel.iter() {
                    let ((ws, user_id, id), tx) = item.pair();
                    if &ws_id == ws && id != &uuid {
                        if tx.is_closed() {
                            closed.push((user_id.clone(), id.clone()));
                        } else if let Err(e) = tx.send(Message::Binary(update.clone())).await {
                            if !tx.is_closed() {
                                error!("on awareness_update error: {}", e);
                            }
                        }
                    }
                }
                for close in closed {
                    let (user_id, id) = close;
                    context.channel.remove(&(ws_id.clone(), user_id, id));
                }
            });
        }
    });
    std::mem::forget(awareness_sub);
}

async fn handle_socket(
    mut socket: WebSocket,
    workspace_id: String,
    context: Arc<Context>,
    user: Option<String>,
) {
    let user_id = if let Some(user_id) = user {
        if let Ok(true) = context
            .db
            .can_read_workspace(user_id.clone(), workspace_id.clone())
            .await
        {
            Some(user_id)
        } else {
            None
        }
    } else {
        None
    };
    let user_id = if let Some(user_id) = user_id {
        user_id
    } else {
        let _ = socket
            .send(ws::Message::Close(Some(CloseFrame {
                code: close_code::POLICY,
                reason: "Unauthorized".into(),
            })))
            .await;
        return;
    };

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

                context
                    .storage
                    .full_migrate(workspace_id.clone(), None)
                    .await;
            }
        });
    }

    let uuid = Uuid::new_v4().to_string();
    context.channel.insert(
        (workspace_id.clone(), user_id.clone(), uuid.clone()),
        tx.clone(),
    );

    if let Ok(init_data) = {
        let ws = context
            .storage
            .create_workspace(workspace_id.clone())
            .await
            .expect("create workspace failed, please check if the workspace_id is valid or not");

        let mut ws = ws.write().await;

        subscribe_handler(context.clone(), &mut ws, uuid.clone(), workspace_id.clone());
        subscribe_metadata_handler(context.clone(), &mut ws, workspace_id.clone());
        subscribe_awareness_handler(context.clone(), &mut ws, uuid.clone(), workspace_id.clone());

        ws.sync_init_message()
    } {
        if tx.send(Message::Binary(init_data)).await.is_err() {
            context.channel.remove(&(workspace_id, user_id, uuid));
            // client disconnected
            return;
        }
    } else {
        context.channel.remove(&(workspace_id, user_id, uuid));
        // client disconnected
        return;
    }

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Binary(binary)) = msg {
            let payload = {
                let workspace = context
                    .storage
                    .get_workspace(workspace_id.clone())
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
                        }
                        // client disconnected
                        return;
                    }
                }
            }
        }
    }

    context.channel.remove(&(workspace_id, user_id, uuid));
}
