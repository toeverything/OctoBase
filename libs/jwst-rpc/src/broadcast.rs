use super::{debug, error, ContextImpl};
use axum::extract::ws::Message;
use dashmap::DashMap;
use jwst::{sync_encode_update, MapSubscription, Workspace};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use y_sync::{
    awareness::{Event, Subscription},
    sync::Message as YMessage,
};
use yrs::{
    updates::encoder::{Encode, Encoder, EncoderV1},
    UpdateSubscription,
};

fn broadcast(
    workspace: String,
    identifier: String,
    update: Vec<u8>,
    channel: DashMap<(String, String), Sender<Message>>,
) {
    tokio::spawn(async move {
        let mut closed = vec![];

        for item in channel.iter() {
            let ((ws, id), tx) = item.pair();
            if &workspace == ws && id != &identifier {
                if tx.is_closed() {
                    closed.push(id.clone());
                } else if let Err(e) = tx.send(Message::Binary(update.clone())).await {
                    if !tx.is_closed() {
                        error!("on awareness_update error: {}", e);
                    }
                }
            }
        }
        for id in closed {
            channel.remove(&(workspace.clone(), id));
        }
    });
}

pub struct Subscriptions {
    _doc: Option<UpdateSubscription>,
    _awareness: Subscription<Event>,
    _metadata: MapSubscription,
}

pub fn subscribe(
    context: Arc<impl ContextImpl<'static>>,
    workspace: &mut Workspace,
    ws_id: String,
    identifier: String,
) -> Subscriptions {
    let awareness = {
        let channel = context.get_channel();
        let ws_id = ws_id.clone();
        let identifier = identifier.clone();
        workspace.on_awareness_update(move |awareness, e| {
            debug!(
                "workspace awareness changed: {}, {:?}",
                ws_id,
                [e.added(), e.updated(), e.removed()].concat()
            );
            if let Ok(update) = awareness
                .update_with_clients([e.added(), e.updated(), e.removed()].concat())
                .map(|update| {
                    let mut encoder = EncoderV1::new();
                    YMessage::Awareness(update).encode(&mut encoder);
                    encoder.to_vec()
                })
            {
                broadcast(ws_id.clone(), identifier.clone(), update, channel.clone());
            }
        })
    };
    let doc = {
        let channel = context.get_channel();
        let ws_id = ws_id.clone();
        let identifier = identifier.clone();
        workspace.observe(move |_, e| {
            debug!("workspace changed: {}, {:?}", ws_id, &e.update);
            let update = sync_encode_update(&e.update);
            broadcast(ws_id.clone(), identifier.clone(), update, channel.clone());
        })
    };
    let metadata = workspace.observe_metadata(move |_, _e| {
        // context
        //     .user_channel
        //     .update_workspace(ws_id.clone(), context.clone());
    });

    Subscriptions {
        _awareness: awareness,
        _doc: doc,
        _metadata: metadata,
    }
}
