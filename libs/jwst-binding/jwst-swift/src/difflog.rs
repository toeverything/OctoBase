use serde::Serialize;
use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    thread::spawn,
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
    pub fn add_receiver(&self, recv: Receiver<Log>) {
        let synced = self.synced.clone();
        spawn(move || {
            while let Ok(last_synced) = recv.recv() {
                synced.lock().unwrap().push(last_synced);
            }
        });
    }

    pub fn pop(&self) -> Vec<Log> {
        let mut synced = self.synced.lock().unwrap();
        let ret = synced.clone();
        synced.clear();
        ret
    }
}
