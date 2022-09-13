use super::*;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Response,
    Json,
};
use lazy_static::lazy_static;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use yrs::{
    updates::{
        decoder::{Decode, Decoder, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
    Doc, StateVector, Update,
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

const MSG_SYNC: usize = 0;
const MSG_SYNC_STEP_1: usize = 0;
const MSG_SYNC_STEP_2: usize = 1;
const MSG_SYNC_UPDATE: usize = 2;

fn write_sync<E: Encoder>(doc: &Doc, encoder: &mut E) {
    encoder.write_var(MSG_SYNC);
}

/// Create a sync step 1 message based on the state of the current shared document.
fn write_step1<E: Encoder>(doc: &Doc, encoder: &mut E) {
    let txn = doc.transact();

    encoder.write_var(MSG_SYNC_STEP_1);
    encoder.write_buf(txn.state_vector().encode_v1());
}

fn write_step2<E: Encoder>(doc: &Doc, sv: &[u8], encoder: &mut E) {
    let txn = doc.transact();
    let remote_sv = StateVector::decode_v1(sv).unwrap();

    encoder.write_var(MSG_SYNC_STEP_2);
    encoder.write_buf(txn.encode_diff_v1(&remote_sv));
}

fn read_sync_message<D: Decoder, E: Encoder>(doc: &Doc, decoder: &mut D, encoder: &mut E) -> usize {
    let msg_type = decoder.read_var().unwrap();
    match msg_type {
        MSG_SYNC_STEP_1 => read_sync_step1(doc, decoder, encoder),
        MSG_SYNC_STEP_2 => read_sync_step2(doc, decoder),
        MSG_SYNC_UPDATE => read_update(doc, decoder),
        other => panic!("Unknown message type: {} to {}", other, doc.client_id),
    }
    msg_type
}

fn read_sync_step1<D: Decoder, E: Encoder>(doc: &Doc, decoder: &mut D, encoder: &mut E) {
    write_step2(doc, decoder.read_buf().unwrap(), encoder)
}

fn read_sync_step2<D: Decoder>(doc: &Doc, decoder: &mut D) {
    let mut txn = doc.transact();

    let update = Update::decode_v1(decoder.read_buf().unwrap()).unwrap();
    txn.apply_update(update);
}

fn read_update<D: Decoder>(doc: &Doc, decoder: &mut D) {
    read_sync_step2(doc, decoder)
}

async fn handle_socket(mut socket: WebSocket, workspace: String) {
    lazy_static! {
        static ref DOC_MAP: Arc<Mutex<HashMap<String, Doc>>> = Arc::new(Mutex::new(HashMap::new()));
    }
    info!("collaboration: {}", workspace);

    let init_data = {
        println!("{}: step0.1", workspace);
        let mut map = DOC_MAP.lock().unwrap();
        println!("{}: step0.2", workspace);
        let doc = if let Some(doc) = map.get(&workspace) {
            doc
        } else {
            let mut doc = Doc::default();
            doc.observe_update_v1(|_, e| {
                println!("on update: {:?}", e.update);
            });
            map.insert(workspace.clone(), doc);
            map.get(&workspace).unwrap()
        };
        println!("{}: step0.3", workspace);

        let mut encoder = EncoderV1::new();
        write_sync(doc, &mut encoder);
        write_step1(&doc, &mut encoder);
        encoder.to_vec()
    };
    println!("{}: step1.1", workspace);
    if socket.send(Message::Binary(init_data)).await.is_err() {
        // client disconnected
        return;
    }

    println!("{}: step1.2", workspace);
    while let Some(msg) = socket.recv().await {
        println!("{}: step2.1", workspace);
        if let Message::Binary(binary) = if let Ok(msg) = msg {
            msg
        } else {
            // client disconnected
            return;
        } {
            let payload = {
                let map = DOC_MAP.lock().unwrap();
                let doc = map.get(&workspace).unwrap();
                let mut encoder = EncoderV1::new();
                let mut decoder = DecoderV1::from(binary.as_slice());

                if decoder.read_info().unwrap() == MSG_SYNC as u8 {
                    write_sync(doc, &mut encoder);
                    read_sync_message(doc, &mut decoder, &mut encoder);
                    let payload = encoder.to_vec();
                    if payload.len() > 1 {
                        Some(payload)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some(binary) = payload {
                println!("{} send {:?}", workspace, binary);
                if socket.send(Message::Binary(binary)).await.is_err() {
                    // client disconnected
                    return;
                }
            }
        }
    }
}
