use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{atomic::AtomicU64, Arc},
};

use axum::{
    extract::{
        ws::{self, close_code, CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Path, Query,
    },
    response::Response,
    routing::get,
    Extension, Router,
};
use dashmap::{mapref::entry::Entry, DashMap};
use futures::{SinkExt, StreamExt};
use jwst::DocStorage;
use serde::Deserialize;
use tokio::sync::{
    mpsc::{channel, Sender},
    RwLock,
};
use y_sync::sync::MessageReader;
use yrs::updates::{decoder::DecoderV1, encoder::Encode};

use crate::{context::Context, model::RefreshToken, utils::URL_SAFE_ENGINE};

pub struct WebSocketContext {
    id: AtomicU64,
    socket: DashMap<i64, RwLock<Vec<AppSocket>>>,
}

struct AppSocket {
    id: u64,
    user_id: i32,
    sender: Sender<Message>,
}

impl WebSocketContext {
    pub fn new() -> Self {
        Self {
            id: AtomicU64::new(0),
            socket: DashMap::new(),
        }
    }
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
    Path(id): Path<i64>,
    Query(Param { token }): Query<Param>,
    ws: WebSocketUpgrade,
) -> Response {
    let user: Option<RefreshToken> = base64::decode_engine(token, &URL_SAFE_ENGINE)
        .ok()
        .and_then(|byte| ctx.decrypt_aes(byte))
        .and_then(|data| serde_json::from_slice(&data).ok());

    let user = if let Some(user) = user {
        if let Ok(true) = ctx.verify_refresh_token(&user).await {
            Some(user.user_id)
        } else {
            None
        }
    } else {
        None
    };

    ws.on_upgrade(move |socket| handle_socket(ctx, user, id, socket))
}

async fn handle_socket(
    ctx: Arc<Context>,
    user: Option<i32>,
    workspace_id: i64,
    mut socket: WebSocket,
) {
    let user_id = if let Some(user_id) = user {
        if let Ok(true) = ctx.can_read_workspace(user_id, workspace_id).await {
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

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = channel(100);

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let id = ctx.ws.id.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

    let ws = AppSocket {
        id,
        user_id,
        sender: tx.clone(),
    };

    match ctx.ws.socket.entry(workspace_id) {
        Entry::Occupied(o) => {
            let mut o = o.get().write().await;

            o.push(ws);
        }
        Entry::Vacant(v) => {
            v.insert(RwLock::new(vec![ws]));
        }
    }

    let task_ctx = ctx.clone();
    let mut recv_task = tokio::spawn(async move {
        let init_message = {
            let doc = if let Some(doc) = task_ctx.doc.get_workspace(workspace_id).await {
                doc
            } else {
                return;
            };

            let doc = doc.read().await;
            if let Ok(msg) = doc.sync_init_message() {
                msg
            } else {
                return;
            }
        };

        if let Err(_) = tx.send(Message::Binary(init_message)).await {
            return;
        }

        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Text(_) => {
                    // TODO: use this as control message
                    continue;
                }
                Message::Binary(update) => {
                    if let Some(socket) = task_ctx.ws.socket.get(&workspace_id) {
                        let mut decoder = DecoderV1::from(&*update);

                        let mut reader = MessageReader::new(&mut decoder);

                        // TODO: subdoc
                        let message = if let Some(msg) = reader.next() {
                            if let Ok(msg) = msg {
                                msg
                            } else {
                                return;
                            }
                        } else {
                            return;
                        };

                        let doc = if let Some(doc) = task_ctx.doc.get_workspace(workspace_id).await
                        {
                            doc
                        } else {
                            break;
                        };

                        let reply = {
                            let mut doc = doc.write().await;
                            use y_sync::sync::{Message, SyncMessage};

                            let reply = match catch_unwind(AssertUnwindSafe(|| {
                                doc.sync_handle_message(message)
                            })) {
                                Ok(Ok(Some(reply))) => reply,
                                Ok(Ok(None)) => continue,
                                _ => return,
                            };

                            if let Message::Sync(SyncMessage::Update(update)) = &reply {
                                let write_res = task_ctx
                                    .doc
                                    .storage
                                    .write_update(workspace_id, update)
                                    .await;

                                match write_res {
                                    Ok(true) => (),
                                    Ok(false) => {
                                        if let Err(_) = task_ctx
                                            .doc
                                            .storage
                                            .write_doc(workspace_id, doc.doc())
                                            .await
                                        {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                };
                            }

                            reply
                        };

                        let socket = socket.read().await;

                        for s in socket.iter() {
                            if s.id != id {
                                let _ = s.sender.send(Message::Binary(reply.encode_v1())).await;
                            }
                        }
                    } else {
                        break;
                    }
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => break,
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    if let Entry::Occupied(mut entry) = ctx.ws.socket.entry(workspace_id) {
        let mut value = entry.get_mut().write().await;

        if value.len() == 1 {
            drop(value);
            entry.remove_entry();
        } else {
            let idx = value.iter().position(|s| s.id == id);

            if let Some(idx) = idx {
                value.swap_remove(idx);
            }
        }
    }
}
