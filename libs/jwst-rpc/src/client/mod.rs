#[cfg(feature = "webrtc")]
mod webrtc;
#[cfg(feature = "websocket")]
mod websocket;

use std::sync::Mutex;

use chrono::Utc;
use tokio::{runtime::Runtime, task::JoinHandle};
#[cfg(feature = "webrtc")]
pub use webrtc::start_webrtc_client_sync;
#[cfg(feature = "websocket")]
pub use websocket::start_websocket_client_sync;

use super::*;

#[derive(Clone, Default)]
pub struct CachedLastSynced {
    synced: Arc<Mutex<Vec<i64>>>,
    _threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl CachedLastSynced {
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
            }
        });
    }

    pub fn pop(&self) -> Vec<i64> {
        let mut synced = self.synced.lock().unwrap();
        let ret = synced.clone();
        synced.clear();
        ret
    }
}

#[cfg(test)]
mod tests {
    use std::thread::spawn;

    use tokio::sync::mpsc::channel;

    use super::*;

    #[test]
    fn test_synced() {
        let synced = CachedLastSynced::default();
        let (tx, rx) = channel::<i64>(1);
        let rt = Arc::new(Runtime::new().unwrap());

        synced.add_receiver(rt.clone(), rx);
        {
            let tx = tx.clone();
            rt.block_on(async {
                tx.send(1).await.unwrap();
                tx.send(2).await.unwrap();
                tx.send(3).await.unwrap();
                sleep(Duration::from_millis(100)).await;
            });
        }
        {
            let synced = synced.clone();
            spawn(move || {
                assert_eq!(synced.pop(), vec![1, 2, 3]);
            })
            .join()
            .unwrap();
        }

        {
            let tx = tx.clone();
            rt.block_on(async {
                tx.send(4).await.unwrap();
                tx.send(5).await.unwrap();
                sleep(Duration::from_millis(100)).await;
            });
        }
        {
            let synced = synced.clone();
            spawn(move || {
                assert_eq!(synced.pop(), vec![4, 5]);
            })
            .join()
            .unwrap();
        }
    }
}
