use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    runtime::Runtime,
    sync::{mpsc::Receiver, Mutex},
    time::sleep,
};

#[derive(Clone, Debug, Serialize)]
pub struct Log {
    content: String,
    timestamp: i64,
    workspace: String,
}

impl Log {
    pub fn new(workspace: String, content: String) -> Self {
        Self {
            content,
            timestamp: chrono::Utc::now().timestamp_millis(),
            workspace,
        }
    }
}

#[derive(Clone, Default)]
pub struct CachedDiffLog {
    synced: Arc<Mutex<Vec<Log>>>,
}

impl CachedDiffLog {
    pub fn add_receiver(&self, mut recv: Receiver<Log>, rt: Arc<Runtime>) {
        let synced = self.synced.clone();

        rt.spawn(async move {
            loop {
                tokio::select! {
                    Some(last_synced) = recv.recv() => {
                        let mut synced = synced.lock().await;
                        synced.push(last_synced);
                    }
                    _ = sleep(Duration::from_secs(5)) => {
                        let mut synced = synced.lock().await;
                        synced.clear();
                    }
                }
            }
        });
    }
}
