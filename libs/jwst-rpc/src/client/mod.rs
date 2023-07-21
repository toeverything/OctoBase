#[cfg(feature = "webrtc")]
mod webrtc;
#[cfg(feature = "websocket")]
mod websocket;

use super::*;
use chrono::Utc;
use std::sync::Mutex;
use tokio::{runtime::Runtime, task::JoinHandle};

#[cfg(feature = "webrtc")]
pub use webrtc::start_webrtc_client_sync;
#[cfg(feature = "websocket")]
pub use websocket::start_websocket_client_sync;

#[derive(Clone)]
pub struct CachedLastSynced {
    synced: Arc<Mutex<Vec<i64>>>,
    _threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl CachedLastSynced {
    pub fn new() -> Self {
        let synced = Arc::new(Mutex::new(Vec::new()));
        Self {
            synced: synced.clone(),
            _threads: Arc::default(),
        }
    }

    pub fn add_receiver_wait_first_update(&self, rt: Arc<Runtime>, recv: Receiver<i64>) {
        self.add_receiver(rt, recv);
        while self.synced.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn add_receiver(&self, rt: Arc<Runtime>, mut recv: Receiver<i64>) {
        let synced = self.synced.clone();
        rt.spawn(async move {
            while let Some(last_synced) = recv.recv().await {
                let mut synced = synced.lock().unwrap();
                if synced.is_empty() || *synced.last().unwrap() != last_synced {
                    synced.push(last_synced);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    pub fn pop(&self) -> Vec<i64> {
        let mut synced = self.synced.lock().unwrap();
        let ret = synced.clone();
        synced.clear();
        ret
    }

    pub fn push(&self, last_synced: i64) {
        let mut synced = self.synced.lock().unwrap();
        if synced.is_empty() || *synced.last().unwrap() != last_synced {
            synced.push(last_synced);
        }
    }
}
