use std::collections::HashMap;

use jwst_codec::{encode_awareness_as_message, encode_update_as_message, encode_update_with_guid};
use jwst_core::Workspace;
use tokio::sync::{broadcast::Sender, RwLock};

use super::*;

#[derive(Clone)]
pub enum BroadcastType {
    // Awareness wrapped in a sync message
    BroadcastAwareness(Vec<u8>),
    // Update wrapped in a sync message
    BroadcastContent(Vec<u8>),
    // Update with guid prefix
    BroadcastRawContent(Vec<u8>),
    CloseUser(String),
    CloseAll,
}

type Broadcast = Sender<BroadcastType>;
pub type BroadcastChannels = RwLock<HashMap<String, Broadcast>>;

pub async fn subscribe(workspace: &Workspace, identifier: String, sender: Broadcast) {
    {
        let sender = sender.clone();
        let workspace_id = workspace.id();

        workspace
            .subscribe_awareness(move |awareness, e| {
                let buffer = match encode_awareness_as_message(e.get_updated(awareness.get_states())) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("failed to write awareness update: {}", e);
                        return;
                    }
                };

                if sender.send(BroadcastType::BroadcastAwareness(buffer.clone())).is_err() {
                    debug!("broadcast channel {workspace_id} has been closed",)
                }
            })
            .await;
    }
    {
        let sender = sender.clone();
        let workspace_id = workspace.id();
        workspace.subscribe_doc(move |update, _history| {
            debug!("workspace {} changed: {}bytes", workspace_id, update.len());

            match encode_update_with_guid(update.to_vec(), workspace_id.clone())
                .and_then(|update_with_guid| encode_update_as_message(update.to_vec()).map(|u| (update_with_guid, u)))
            {
                Ok((broadcast_update, sendable_update)) => {
                    if sender
                        .send(BroadcastType::BroadcastRawContent(broadcast_update))
                        .is_err()
                    {
                        debug!("broadcast channel {workspace_id} has been closed",)
                    }

                    if sender.send(BroadcastType::BroadcastContent(sendable_update)).is_err() {
                        debug!("broadcast channel {workspace_id} has been closed",)
                    }
                }
                Err(e) => {
                    debug!("failed to encode update: {}", e);
                }
            }
        });
    };

    let workspace_id = workspace.id();
    tokio::spawn(async move {
        let mut rx = sender.subscribe();
        loop {
            tokio::select! {
                Ok(msg) = rx.recv()=> {
                    match msg {
                        BroadcastType::CloseUser(user) if user == identifier => break,
                        BroadcastType::CloseAll => break,
                        _ => {}
                    }
                },
                _ = sleep(Duration::from_millis(100)) => {
                    let count = sender.receiver_count();
                    if count < 1 {
                        break;
                    }
                }
            }
        }
        debug!("broadcast channel {workspace_id} has been closed");
    });
}
