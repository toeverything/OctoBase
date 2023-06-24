use super::Workspace;
use std::{
    collections::HashSet,
    sync::{
        atomic::{
            AtomicBool,
            Ordering::{Acquire, Release},
        },
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::{
    runtime::{self, Runtime},
    sync::RwLock,
    time::sleep,
};
use tracing::debug;

type CallbackFn = Arc<RwLock<Option<Arc<Box<dyn Fn(String, Vec<String>) + Send + Sync>>>>>;
pub struct BlockObserverConfig {
    pub(crate) workspace_id: String,
    pub(super) callback: CallbackFn,
    pub(super) runtime: Arc<Runtime>,
    pub(crate) tx: Sender<String>,
    pub(super) rx: Arc<Mutex<Receiver<String>>>,
    // modified_block_ids can be consumed either automatically by callback or
    // manually retrieval identified by is_manually_tracking_block_changes
    pub(super) modified_block_ids: Arc<RwLock<HashSet<String>>>,
    pub(crate) handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub(crate) is_manually_tracking_block_changes: Arc<AtomicBool>,
    pub(crate) is_observing: Arc<AtomicBool>,
}

impl BlockObserverConfig {
    pub fn new(workspace_id: String) -> Self {
        let runtime = Arc::new(
            runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_time()
                .enable_io()
                .build()
                .unwrap(),
        );
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let modified_block_ids = Arc::new(RwLock::new(HashSet::new()));
        let callback = Arc::new(RwLock::new(None));
        let mut block_observer_config = BlockObserverConfig {
            workspace_id,
            callback: callback.clone(),
            runtime,
            tx,
            rx: Arc::new(Mutex::new(rx)),
            modified_block_ids,
            handle: Arc::new(Mutex::new(None)),
            is_manually_tracking_block_changes: Arc::default(),
            is_observing: Arc::default(),
        };

        block_observer_config.handle = Arc::new(Mutex::new(Some(
            block_observer_config.start_callback_thread(),
        )));

        block_observer_config
    }

    pub fn is_consuming(&self) -> bool {
        self.is_observing.load(Acquire)
    }

    pub fn set_callback(&self, cb: Arc<Box<dyn Fn(String, Vec<String>) + Send + Sync>>) {
        self.is_observing.store(true, Release);
        let callback = self.callback.clone();
        self.runtime.spawn(async move {
            *callback.write().await = Some(cb);
        });
        self.is_manually_tracking_block_changes
            .store(false, Release);
    }

    pub fn set_tracking_block_changes(&self, if_tracking: bool) {
        self.is_observing.store(true, Release);
        self.is_manually_tracking_block_changes
            .store(if_tracking, Release);
        let callback = self.callback.clone();
        self.runtime.spawn(async move {
            *callback.write().await = None;
        });
    }

    pub fn retrieve_modified_blocks(&self) -> HashSet<String> {
        let modified_block_ids = self.modified_block_ids.clone();
        self.runtime.block_on(async move {
            let mut guard = modified_block_ids.write().await;
            let modified_block_ids = guard.clone();
            guard.clear();
            modified_block_ids
        })
    }

    fn start_callback_thread(&self) -> JoinHandle<()> {
        let rx = self.rx.clone();
        let modified_block_ids = self.modified_block_ids.clone();
        let callback = self.callback.clone();
        let runtime = self.runtime.clone();
        let is_tracking_block_changes = self.is_manually_tracking_block_changes.clone();
        let workspace_id = self.workspace_id.clone();
        std::thread::spawn(move || {
            let rx = rx.lock().unwrap();
            let rt = runtime.clone();
            while let Ok(block_id) = rx.recv() {
                debug!("received block change from {}", block_id);
                let modified_block_ids = modified_block_ids.clone();
                let callback = callback.clone();
                let is_tracking_block_changes = is_tracking_block_changes.clone();
                let workspace_id = workspace_id.clone();
                rt.spawn(async move {
                    if let Some(callback) = callback.read().await.as_ref() {
                        let mut guard = modified_block_ids.write().await;
                        guard.insert(block_id);
                        drop(guard);
                        // merge changed blocks in between 200 ms
                        sleep(Duration::from_millis(200)).await;
                        let mut guard = modified_block_ids.write().await;
                        if !guard.is_empty() {
                            let block_ids = guard
                                .iter()
                                .map(|item| item.to_owned())
                                .collect::<Vec<String>>();
                            debug!("invoking callback with block ids: {:?}", block_ids);
                            callback(workspace_id, block_ids);
                            guard.clear();
                        }
                    } else if is_tracking_block_changes.load(Acquire) {
                        let mut guard = modified_block_ids.write().await;
                        guard.insert(block_id);
                    }
                });
            }
        })
    }
}

impl Workspace {
    pub fn init_block_observer_config(&mut self) {
        self.block_observer_config = Some(Arc::new(BlockObserverConfig::new(self.id())));
    }

    pub fn set_callback(&self, cb: Arc<Box<dyn Fn(String, Vec<String>) + Send + Sync>>) -> bool {
        if let Some(block_observer_config) = self.block_observer_config.clone() {
            if !block_observer_config.is_consuming() {
                block_observer_config.set_callback(cb);
            }
            return true;
        }
        false
    }

    pub fn set_tracking_block_changes(&self, if_tracking: bool) {
        if let Some(block_observer_config) = self.block_observer_config.clone() {
            block_observer_config.set_tracking_block_changes(if_tracking);
        }
    }

    pub fn retrieve_modified_blocks(&self) -> Option<HashSet<String>> {
        self.block_observer_config
            .clone()
            .and_then(|block_observer_config| {
                block_observer_config
                    .is_manually_tracking_block_changes
                    .load(Acquire)
                    .then(|| block_observer_config.retrieve_modified_blocks())
            })
    }

    pub fn get_tokio_runtime(&self) -> Option<Arc<Runtime>> {
        self.block_observer_config
            .clone()
            .map(|block_observer_config| block_observer_config.runtime.clone())
    }
}
