use super::{debug, error, info, trace, ChannelItem, ContextImpl};
use axum::extract::ws::Message;
use jwst::{sync_encode_update, MapSubscription, Workspace};
use std::sync::Arc;
use y_sync::{
    awareness::{Event, Subscription},
    sync::Message as YMessage,
};
use yrs::{
    updates::encoder::{Encode, Encoder, EncoderV1},
    UpdateSubscription,
};

fn broadcast(
    current_item: ChannelItem,
    update: Vec<u8>,
    context: Arc<impl ContextImpl<'static> + Send + Sync + 'static>,
) {
    let context = context.clone();
    tokio::spawn(async move {
        let mut closed = vec![];

        for item in context.get_channel().iter() {
            let (item, tx) = item.pair();
            if current_item.workspace == item.workspace && current_item.uuid != item.uuid {
                if tx.is_closed() {
                    closed.push(item.clone());
                } else if let Err(e) = tx.send(Message::Binary(update.clone())).await {
                    if !tx.is_closed() {
                        error!("on awareness_update error: {}", e);
                    }
                }
            }
        }
        for item in closed {
            context.get_channel().remove(&item);
        }
    });
}

pub struct Subscriptions {
    _doc: Option<UpdateSubscription>,
    _awareness: Subscription<Event>,
    _metadata: MapSubscription,
}

pub fn subscribe(
    context: Arc<impl ContextImpl<'static> + Send + Sync + 'static>,
    workspace: &mut Workspace,
    item: &ChannelItem,
) -> Subscriptions {
    let awareness = {
        let context = context.clone();
        let item = item.clone();
        workspace.on_awareness_update(move |awareness, e| {
            trace!(
                "workspace awareness changed: {}, {:?}",
                item.workspace,
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
                broadcast(item.clone(), update, context.clone());
            }
        })
    };
    let doc = {
        let context = context.clone();
        let item = item.clone();
        workspace.observe(move |_, e| {
            debug!("workspace changed: {}, {:?}", item.workspace, &e.update);
            let update = sync_encode_update(&e.update);
            broadcast(item.clone(), update, context.clone());
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
