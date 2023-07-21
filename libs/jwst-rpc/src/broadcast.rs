use super::*;
use jwst::Workspace;
use jwst_codec::{
    write_sync_message, CrdtWriter, DocMessage, JwstCodecError, JwstCodecResult, RawEncoder,
    SyncMessage,
};
use lru_time_cache::LruCache;
use std::{collections::HashMap, io::Write, sync::Mutex};
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

fn encode_sendable_update(update: Vec<u8>) -> JwstCodecResult<Vec<u8>> {
    let mut buffer = Vec::new();
    write_sync_message(&mut buffer, &SyncMessage::Doc(DocMessage::Update(update)))
        .map_err(|e| JwstCodecError::InvalidWriteBuffer(e.to_string()))?;

    Ok(buffer)
}

fn encode_update_with_guid<S: AsRef<str>>(update: Vec<u8>, guid: S) -> JwstCodecResult<Vec<u8>> {
    let mut encoder = RawEncoder::default();
    encoder.write_var_string(guid)?;
    let mut buffer = encoder.into_inner();

    buffer
        .write_all(&update)
        .map_err(|e| JwstCodecError::InvalidWriteBuffer(e.to_string()))?;

    Ok(buffer)
}

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
                let mut buffer = Vec::new();
                if let Err(e) = write_sync_message(
                    &mut buffer,
                    &SyncMessage::Awareness(e.get_updated(awareness.get_states())),
                ) {
                    error!("failed to write awareness update: {}", e);
                    return;
                }

                let mut dedup_cache = dedup_cache.lock().unwrap_or_else(|e| e.into_inner());
                if !dedup_cache.contains_key(&buffer) {
                    if sender
                        .send(BroadcastType::BroadcastAwareness(buffer.clone()))
                        .is_err()
                    {
                        debug!("broadcast channel {workspace_id} has been closed",)
                    }
                    dedup_cache.insert(buffer, ());
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

            match encode_update_with_guid(e.update.clone(), workspace_id.clone())
                .and_then(|update| encode_sendable_update(update.clone()).map(|u| (update, u)))
            {
                Ok((broadcast_update, sendable_update)) => {
            if sender
                        .send(BroadcastType::BroadcastRawContent(broadcast_update))
                .is_err()
            {
                        println!("broadcast channel {workspace_id} has been closed",)
            }

            if sender
                        .send(BroadcastType::BroadcastContent(sendable_update))
                .is_err()
            {
                        println!("broadcast channel {workspace_id} has been closed",)
                    }
                }
                Err(e) => {
                    println!("failed to encode update: {}", e);
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
