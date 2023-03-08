use super::{debug, trace};
use jwst::{info, sync_encode_update, MapSubscription, Workspace};
use std::collections::HashMap;
use tokio::sync::{broadcast::Sender, RwLock};
use y_sync::{
    awareness::{Event, Subscription},
    sync::Message as YMessage,
};
use yrs::{
    updates::encoder::{Encode, Encoder, EncoderV1},
    UpdateSubscription,
};

#[derive(Clone)]
pub enum BroadcastType {
    Broadcast(Vec<u8>),
    CloseUser(String),
    CloseAll,
}

type Broadcast = Sender<BroadcastType>;
pub type BroadcastChannels = RwLock<HashMap<String, Broadcast>>;

pub struct Subscriptions {
    _doc: Option<UpdateSubscription>,
    _awareness: Subscription<Event>,
    _metadata: MapSubscription,
}

pub fn subscribe(workspace: &mut Workspace, sender: Broadcast) -> Subscriptions {
    let awareness = {
        let sender = sender.clone();
        let workspace_id = workspace.id();
        workspace.on_awareness_update(move |awareness, e| {
            trace!(
                "workspace awareness changed: {}, {:?}",
                workspace_id,
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
                if sender.send(BroadcastType::Broadcast(update)).is_err() {
                    info!("broadcast channel {workspace_id} has been closed",)
                }
            }
        })
    };
    let doc = {
        let workspace_id = workspace.id();
        workspace.observe(move |_, e| {
            debug!(
                "workspace {} changed: {}bytes",
                workspace_id,
                &e.update.len()
            );
            let update = sync_encode_update(&e.update);
            if sender.send(BroadcastType::Broadcast(update)).is_err() {
                info!("broadcast channel {workspace_id} has been closed",)
            }
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
