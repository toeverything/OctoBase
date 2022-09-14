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
use dashmap::mapref::entry::Entry;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{mpsc::channel, Mutex};
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

pub async fn upgrade_handler(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(["AFFiNE"])
        .on_upgrade(async move |socket| handle_socket(socket, workspace, context.clone()).await)
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
        let entry = context.doc.entry(workspace.clone());
        let doc = match entry {
            Entry::Occupied(value) => value.into_ref(),
            Entry::Vacant(entry) => {
                let mut doc = Doc::with_options(Options {
                    skip_gc: true,
                    ..Default::default()
                });

                let mut db = init("updates").await.unwrap();

                let updates = db.all(0).await;

                let mut txn = doc.transact();
                for update in updates.unwrap() {
                    if let Ok(update) = Update::decode_v1(&update.blob) {
                        txn.apply_update(update);
                    }
                }
                txn.commit();

                let ws = workspace.clone();
                let db = Arc::new(Mutex::new(db));

                {
                    let context = context.clone();
                    let sub = doc.observe_update_v1(move |_, e| {
                        let mut encoder = EncoderV1::new();
                        write_sync(&mut encoder);
                        write_update(&e.update, &mut encoder);
                        let update = encoder.to_vec();

                        let uuid = uuid.clone();
                        let workspace = ws.clone();

                        let db = db.clone();
                        let context = context.clone();
                        tokio::spawn(async move {
                            let mut closed = vec![];
                            for item in context.channel.iter() {
                                let ((ws, id), tx) = item.pair();
                                if workspace.as_str() == ws.as_str() && id.as_str() != uuid.as_str()
                                {
                                    if tx.is_closed() {
                                        closed.push(id.clone());
                                    } else {
                                        db.lock().await.insert(&update).await.unwrap();
                                        if let Err(e) =
                                            tx.send(Message::Binary(update.clone())).await
                                        {
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

                entry.insert(Mutex::new(doc))
            }
        };

        let mut encoder = EncoderV1::new();
        write_sync(&mut encoder);
        let doc = doc.value().lock().await;
        write_step1(&doc, &mut encoder);

        encoder.to_vec()
    };

    if tx.send(Message::Binary(init_data)).await.is_err() {
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
                    .and_then(|_| {
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
}
