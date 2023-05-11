use super::*;
use jwst::{sync_encode_update, Workspace};
use jwst_codec::{convert_awareness_update, write_sync_message};
use lru_time_cache::LruCache;
use std::{collections::HashMap, sync::Mutex};
use tokio::sync::{broadcast::Sender, RwLock};

#[derive(Clone)]
pub enum BroadcastType {
    BroadcastAwareness(Vec<u8>),
    BroadcastContent(Vec<u8>),
    BroadcastRawContent(Vec<u8>),
    CloseUser(String),
    CloseAll,
}

type Broadcast = Sender<BroadcastType>;
pub type BroadcastChannels = RwLock<HashMap<String, Broadcast>>;

pub async fn subscribe(workspace: &mut Workspace, identifier: String, sender: Broadcast) {
    {
        let sender = sender.clone();
        let workspace_id = workspace.id();

        let dedup_cache = Arc::new(Mutex::new(LruCache::with_expiry_duration_and_capacity(
            Duration::from_micros(100),
            128,
        )));

        workspace
            .on_awareness_update(move |awareness, e| {
                trace!(
                    "workspace awareness changed: {}, {:?}",
                    workspace_id,
                    [e.added(), e.updated(), e.removed()].concat()
                );
                if let Some(update) = awareness
                    .update_with_clients([e.added(), e.updated(), e.removed()].concat())
                    .ok()
                    .and_then(|update| {
                        let mut buffer = Vec::new();
                        match write_sync_message(&mut buffer, &convert_awareness_update(update)) {
                            Ok(_) => Some(buffer),
                            Err(e) => {
                                error!("failed to write awareness update: {}", e);
                                None
                            }
                        }
                    })
                {
                    let mut dedup_cache = dedup_cache.lock().unwrap_or_else(|e| e.into_inner());
                    if !dedup_cache.contains_key(&update) {
                        if sender
                            .send(BroadcastType::BroadcastAwareness(update.clone()))
                            .is_err()
                        {
                            debug!("broadcast channel {workspace_id} has been closed",)
                        }
                        dedup_cache.insert(update, ());
                    }
                }
            })
            .await;
    }
    {
        let sender = sender.clone();
        let workspace_id = workspace.id();
        workspace.observe(move |_, e| {
            trace!(
                "workspace {} changed: {}bytes",
                workspace_id,
                &e.update.len()
            );

            if sender
                .send(BroadcastType::BroadcastRawContent(e.update.clone()))
                .is_err()
            {
                debug!("broadcast channel {workspace_id} has been closed",)
            }
            let update = sync_encode_update(&e.update);
            if sender
                .send(BroadcastType::BroadcastContent(update))
                .is_err()
            {
                debug!("broadcast channel {workspace_id} has been closed",)
            }
        });
    };
    // let metadata = workspace.observe_metadata(move |_, _e| {
    //     // context
    //     //     .user_channel
    //     //     .update_workspace(ws_id.clone(), context.clone());
    // });

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
