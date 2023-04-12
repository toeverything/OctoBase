use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use anyhow::Context;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::debug;

type CallbackFn = Arc<RwLock<Option<Box<dyn Fn(Vec<String>) + Send + Sync>>>>;

pub struct BlockObserverConfig {
    pub(crate) callback: CallbackFn,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) tx: std::sync::mpsc::Sender<String>,
    pub(crate) rx: Arc<Mutex<std::sync::mpsc::Receiver<String>>>,
    pub(crate) modified_block_ids: Arc<RwLock<HashSet<String>>>,
    pub(crate) handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl BlockObserverConfig {
    pub fn new() -> Self {
        let runtime = Arc::new(runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_time()
            .build()
            .context("Failed to create runtime")
            .unwrap());
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let modified_block_ids = Arc::new(RwLock::new(HashSet::new()));
        let callback = Arc::new(RwLock::new(None));

        let mut block_observer_config = BlockObserverConfig {
            callback: callback.clone(),
            runtime,
            tx,
            rx: Arc::new(Mutex::new(rx)),
            modified_block_ids,
            handle: Arc::new(Mutex::new(None)),
        };

        block_observer_config.handle = Arc::new(
            Mutex::new(Some(block_observer_config.start_callback_thread())));

        block_observer_config
    }

    pub fn set_callback(&self, cb: Box<dyn Fn(Vec<String>) + Send + Sync>) {
        let callback = self.callback.clone();
        self.runtime.spawn(async move {
            *callback.write().await = Some(cb);
        });
    }

    fn start_callback_thread(&self) -> JoinHandle<()> {
        let rx = self.rx.clone();
        let modified_block_ids = self.modified_block_ids.clone();
        let callback = self.callback.clone();
        let runtime = self.runtime.clone();
        std::thread::spawn(move || {
            let rx = rx.lock().unwrap();
            let rt = runtime.clone();
            while let Ok(block_id) = rx.recv() {
                debug!("received block change from {}", block_id);
                let modified_block_ids = modified_block_ids.clone();
                let callback = callback.clone();
                rt.spawn(async move {
                    if let Some(callback) = callback.read().await.as_ref() {
                        let mut guard = modified_block_ids.write().await;
                        guard.insert(block_id);
                        drop(guard);
                        // merge changed blocks in between 200 ms
                        sleep(Duration::from_millis(200)).await;
                        let mut guard = modified_block_ids.write().await;
                        if !guard.is_empty() {
                            let block_ids = guard.iter().map(|item| item.to_owned()).collect::<Vec<String>>();
                            debug!("invoking callback with block ids: {:?}", block_ids);
                            callback(block_ids);
                            guard.clear();
                        }
                    }
                });
            }
        })
    }
}

impl Default for BlockObserverConfig {
    fn default() -> Self {
        BlockObserverConfig::new()
    }
}