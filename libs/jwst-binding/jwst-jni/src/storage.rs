use crate::Workspace;
use android_logger::Config;
use futures::TryFutureExt;
use jwst::{error, JwstError, LevelFilter};
use jwst_rpc::{start_sync_thread1, BroadcastChannels, RpcContextImpl, SyncState};
use jwst_storage::{JwstStorage as AutoStorage, JwstStorageResult};
use log::log::warn;
use std::sync::{Arc, RwLock};
use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct JwstStorage {
    storage: Arc<AutoStorage>,
    channel: Arc<BroadcastChannels>,
    error: Option<String>,
    pub(crate) sync_state: Arc<RwLock<SyncState>>,
}

impl JwstStorage {
    pub fn new(path: String) -> Self {
        Self::new_with_logger_level(path, "debug".to_string())
    }

    pub fn new_with_logger_level(path: String, level: String) -> Self {
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
                AutoStorage::new(&format!("sqlite:{path}?mode=rwc")).or_else(|e| {
                    warn!(
                        "Failed to open storage, falling back to memory storage: {}",
                        e
                    );
                    AutoStorage::new("sqlite::memory:")
                }),
            )
            .unwrap();

        Self {
            storage: Arc::new(storage),
            channel: Arc::default(),
            error: None,
            sync_state: Arc::new(RwLock::new(SyncState::Offline)),
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    pub fn is_offline(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Offline)
    }

    pub fn is_initialized(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Initialized)
    }

    pub fn is_syncing(&self) -> bool {
        let sync_state = self.sync_state.read().unwrap();
        matches!(*sync_state, SyncState::Syncing)
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
        match *sync_state {
            SyncState::Offline => "offline".to_string(),
            SyncState::Syncing => "syncing".to_string(),
            SyncState::Initialized => "initialized".to_string(),
            SyncState::Finished => "finished".to_string(),
            SyncState::Error(_) => "Error".to_string(),
        }
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

    fn sync(&self, workspace_id: String, remote: String) -> JwstStorageResult<Workspace> {
        let rt = Arc::new(Runtime::new().map_err(JwstError::Io)?);

        if !remote.is_empty() {
            start_sync_thread1(
                rt.clone(),
                Arc::new(self.clone()),
                self.sync_state.clone(),
                remote,
                workspace_id.clone(),
            );
        }

        Ok(Workspace {
            workspace: rt.block_on(self.get_workspace(&workspace_id))?,
        })
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
