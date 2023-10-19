use std::sync::{Arc, RwLock};

use android_logger::Config;
use futures::TryFutureExt;
use jwst_rpc::{start_websocket_client_sync, BroadcastChannels, CachedLastSynced, RpcContextImpl, SyncState};
use jwst_storage::{BlobStorageType, JwstStorage as AutoStorage, JwstStorageError, JwstStorageResult};
use nanoid::nanoid;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc::channel,
};

use super::*;

#[derive(Clone)]
pub struct JwstStorage {
    storage: Arc<AutoStorage>,
    channel: Arc<BroadcastChannels>,
    error: Option<String>,
    sync_state: Arc<RwLock<SyncState>>,
    last_sync: CachedLastSynced,
}

impl JwstStorage {
    pub fn new(path: String) -> Self {
        Self::new_with_log_level(path, "info".to_string())
    }

    pub fn new_with_log_level(path: String, level: String) -> Self {
        let level = match level.to_lowercase().as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Debug,
        };
        android_logger::init_once(Config::default().with_max_level(level).with_tag("jwst"));

        let rt = Runtime::new().unwrap();

        let storage = rt
            .block_on(
                AutoStorage::new_with_migration(&format!("sqlite:{path}?mode=rwc"), BlobStorageType::DB).or_else(|e| {
                    warn!("Failed to open storage, falling back to memory storage: {}", e);
                    AutoStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
                }),
            )
            .unwrap();

        Self {
            storage: Arc::new(storage),
            channel: Arc::default(),
            error: None,
            sync_state: Arc::new(RwLock::new(SyncState::Offline)),
            last_sync: CachedLastSynced::default(),
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    pub fn is_offline(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Offline)
    }

    pub fn is_connected(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Connected)
    }

    pub fn is_finished(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Finished)
    }

    pub fn is_error(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Error(_))
    }

    pub fn get_sync_state(&self) -> String {
        let sync_state = self.sync_state.read().unwrap();
        match sync_state.clone() {
            SyncState::Offline => "offline".to_string(),
            SyncState::Connected => "connected".to_string(),
            SyncState::Finished => "finished".to_string(),
            SyncState::Error(e) => format!("Error: {e}"),
        }
    }

    pub fn import(&mut self, workspace_id: String, data: &[u8]) -> bool {
        match self.import_workspace(workspace_id, data) {
            Ok(_) => true,
            Err(e) => {
                let error = format!("Failed to init workspace: {:?}", e);
                error!("{}", error);
                self.error = Some(error);
                false
            }
        }
    }

    fn import_workspace(&self, workspace_id: String, data: &[u8]) -> JwstStorageResult {
        let rt = Arc::new(
            Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("jwst-jni-init")
                .build()
                .map_err(JwstStorageError::SyncThread)?,
        );
        rt.block_on(self.storage.init_workspace(workspace_id, data.to_vec()))
    }

    pub fn export(&mut self, workspace_id: String) -> Vec<u8> {
        match self.export_workspace(workspace_id) {
            Ok(data) => data,
            Err(e) => {
                let error = format!("Failed to export workspace: {:?}", e);
                error!("{}", error);
                self.error = Some(error);
                vec![]
            }
        }
    }

    fn export_workspace(&self, workspace_id: String) -> JwstStorageResult<Vec<u8>> {
        let rt = Arc::new(
            Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("jwst-jni-export")
                .build()
                .map_err(JwstStorageError::SyncThread)?,
        );
        rt.block_on(self.storage.export_workspace(workspace_id))
    }

    pub fn connect(&mut self, workspace_id: String, remote: String) -> Option<Workspace> {
        match self.sync(workspace_id, remote) {
            Ok(workspace) => Some(workspace),
            Err(e) => {
                error!("Failed to connect to workspace: {:?}", e);
                self.error = Some(e.to_string());
                None
            }
        }
    }

    fn sync(&mut self, workspace_id: String, remote: String) -> JwstStorageResult<Workspace> {
        let rt = Arc::new(
            Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("jwst-jni")
                .build()
                .map_err(JwstStorageError::SyncThread)?,
        );
        let is_offline = remote.is_empty();

        let workspace = rt.block_on(async { self.get_workspace(&workspace_id).await });

        match workspace {
            Ok(mut workspace) => {
                warn!("is_offline: {}, remote: {}", is_offline, remote);
                if is_offline {
                    let identifier = nanoid!();
                    let (last_synced_tx, last_synced_rx) = channel::<i64>(128);
                    self.last_sync.add_receiver(rt.clone(), last_synced_rx);

                    rt.block_on(async {
                        self.join_broadcast(&mut workspace, identifier.clone(), last_synced_tx)
                            .await;
                    });
                } else {
                    self.last_sync = start_websocket_client_sync(
                        rt.clone(),
                        Arc::new(self.clone()),
                        self.sync_state.clone(),
                        remote,
                        workspace_id.clone(),
                    );
                }

                Ok(Workspace { workspace, runtime: rt })
            }
            Err(e) => Err(e),
        }
    }

    pub fn get_last_synced(&self) -> Vec<i64> {
        self.last_sync.pop()
    }
}

impl RpcContextImpl<'_> for JwstStorage {
    fn get_storage(&self) -> &AutoStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}
