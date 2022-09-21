use super::*;
use crate::sync::*;
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
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use yrs::{
    updates::{
        decoder::{Decoder, DecoderV1},
        encoder::{Encoder, EncoderV1},
    },
    Doc,
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

pub async fn upgrade_handler(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(["AFFiNE"])
        .on_upgrade(|socket| async move { handle_socket(socket, workspace, context.clone()).await })
}

pub fn subscribe_handler(context: Arc<Context>, doc: &mut Doc, uuid: String, workspace: String) {
    let sub = doc.observe_update_v1(move |_, e| {
        let mut encoder = EncoderV1::new();
        write_sync(&mut encoder);
        write_update(&e.update, &mut encoder);
        let update = encoder.to_vec();

        let context = context.clone();
        let uuid = uuid.clone();
        let workspace = workspace.clone();
        tokio::spawn(async move {
            let mut closed = vec![];

            for item in context.channel.iter() {
                let ((ws, id), tx) = item.pair();
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
            for id in closed {
                context.channel.remove(&(workspace.clone(), id));
            }
        });
    });
    std::mem::forget(sub);
}

async fn handle_socket(socket: WebSocket, workspace: String, context: Arc<Context>) {
    info!("collaboration: {}", workspace);

    let (mut socket_tx, mut socket_rx) = socket.split();
    let (tx, mut rx) = channel(100);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = socket_tx.send(msg).await {
                error!("send error: {}", e);
                break;
            }
        }
    });

    let uuid = Uuid::new_v4().to_string();
    context
        .channel
        .insert((workspace.clone(), uuid.clone()), tx.clone());

    let init_data = {
        utils::init_doc(context.clone(), workspace.clone()).await;

        let doc = context.doc.get(&workspace).unwrap();
        let mut doc = doc.lock().await;

        subscribe_handler(context.clone(), &mut doc, uuid.clone(), workspace.clone());

        let mut encoder = EncoderV1::new();
        write_sync(&mut encoder);
        write_step1(&doc, &mut encoder);

        encoder.to_vec()
    };

    if tx.send(Message::Binary(init_data)).await.is_err() {
        context.channel.remove(&(workspace, uuid));
        // client disconnected
        return;
    }

    while let Some(msg) = socket_rx.next().await {
        if let Ok(Message::Binary(binary)) = msg {
            let payload = {
                let doc = context.doc.get(&workspace).unwrap();
                let doc = doc.value().lock().await;
                let mut encoder = EncoderV1::new();
                let mut decoder = DecoderV1::from(binary.as_slice());

                if decoder.read_info().unwrap() == MSG_SYNC as u8 {
                    use std::panic::{catch_unwind, AssertUnwindSafe};
                    catch_unwind(AssertUnwindSafe(|| {
                        write_sync(&mut encoder);
                        read_sync_message(&doc, &mut decoder, &mut encoder);
                    }))
                    .ok()
                    .and_then(|()| {
                        let payload = encoder.to_vec();
                        if payload.len() > 1 {
                            Some(payload)
                        } else {
                            None
                        }
                    })
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

    context.channel.remove(&(workspace, uuid));
}
