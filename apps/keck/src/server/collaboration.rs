use super::*;
use crate::sync::*;
use async_lock::Mutex;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Response,
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::mpsc::{channel, Sender};
use yrs::{
    updates::{
        decoder::{Decoder, DecoderV1},
        encoder::{Encoder, EncoderV1},
    },
    Doc, Options,
};

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub async fn auth_handler(Path(workspace): Path<String>) -> Json<WebSocketAuthentication> {
    info!("auth: {}", workspace);
    Json(WebSocketAuthentication {
        protocol: "AFFiNE".to_owned(),
    })
}

pub async fn upgrade_handler(Path(workspace): Path<String>, ws: WebSocketUpgrade) -> Response {
    ws.protocols(["AFFiNE"])
        .on_upgrade(async move |socket| handle_socket(socket, workspace).await)
}

async fn handle_socket(socket: WebSocket, workspace: String) {
    lazy_static! {
        static ref DOC_MAP: Mutex<HashMap<String, Doc>> = Mutex::new(HashMap::new());
        static ref CHANNEL_MAP: Mutex<HashMap<(String, String), Sender<Message>>> =
            Mutex::new(HashMap::new());
    }
    info!("collaboration: {}", workspace);

    let (mut socket_tx, mut socket_rx) = socket.split();
    let (tx, mut rx) = channel(100);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            socket_tx.send(msg).await.unwrap();
        }
    });

    let uuid = Uuid::new_v4().to_string();
    CHANNEL_MAP
        .lock()
        .await
        .insert((workspace.clone(), uuid.clone()), tx.clone());

    let init_data = {
        let mut map = DOC_MAP.lock().await;

        let doc = if let Some(doc) = map.get_mut(&workspace) {
            doc
        } else {
            let mut doc = Doc::with_options(Options {
                skip_gc: true,
                ..Default::default()
            });

            let ws = workspace.clone();
            let sub = doc.observe_update_v1(move |_, e| {
                let mut encoder = EncoderV1::new();
                write_sync(&mut encoder);
                write_update(&e.update, &mut encoder);
                let update = encoder.to_vec();

                let uuid = uuid.clone();
                let workspace = ws.clone();

                tokio::spawn(async move {
                    let mut closed = vec![];
                    for ((ws, id), tx) in CHANNEL_MAP.lock().await.iter() {
                        if workspace.as_str() == ws.as_str() && id.as_str() != uuid.as_str() {
                            if tx.is_closed() {
                                closed.push(id.clone());
                            } else {
                                if let Err(e) = tx.send(Message::Binary(update.clone())).await {
                                    println!("on observe_update_v1 error: {}", e);
                                }
                            }
                        }
                    }
                    let mut map = CHANNEL_MAP.lock().await;
                    for id in closed {
                        map.remove(&(workspace.clone(), id));
                    }
                });
            });
            std::mem::forget(sub);

            map.insert(workspace.clone(), doc);
            map.get_mut(&workspace).unwrap()
        };

        let mut encoder = EncoderV1::new();
        write_sync(&mut encoder);
        write_step1(&doc, &mut encoder);

        encoder.to_vec()
    };

    if tx.send(Message::Binary(init_data)).await.is_err() {
        // client disconnected
        return;
    }

    while let Some(msg) = socket_rx.next().await {
        if let Message::Binary(binary) = if let Ok(msg) = msg {
            msg
        } else {
            // client disconnected
            return;
        } {
            let payload = {
                let map = DOC_MAP.lock().await;
                let doc = map.get(&workspace).unwrap();
                let mut encoder = EncoderV1::new();
                let mut decoder = DecoderV1::from(binary.as_slice());

                if decoder.read_info().unwrap() == MSG_SYNC as u8 {
                    use std::panic::{catch_unwind, AssertUnwindSafe};
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        write_sync(&mut encoder);
                        read_sync_message(doc, &mut decoder, &mut encoder);
                    }));
                    if result.is_err() {
                        None
                    } else {
                        let payload = encoder.to_vec();
                        if payload.len() > 1 {
                            Some(payload)
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            };
            if let Some(binary) = payload {
                if let Err(e) = tx.send(Message::Binary(binary)).await {
                    println!("on send error: {:?}", e);
                    // client disconnected
                    return;
                }
            }
        }
    }
}
