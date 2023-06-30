use crate::Workspace;
use android_logger::Config;
use futures::TryFutureExt;
use jwst::{error, JwstError, LevelFilter};
use jwst_rpc::{start_client_sync, BroadcastChannels, RpcContextImpl, SyncState};
use jwst_storage::{BlobStorageType, JwstStorage as AutoStorage, JwstStorageResult};
use log::log::warn;
use nanoid::nanoid;
use std::sync::{Arc, RwLock};
use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct JwstStorage {
    storage: Arc<AutoStorage>,
    channel: Arc<BroadcastChannels>,
    error: Option<String>,
    sync_state: Arc<RwLock<SyncState>>,
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
                AutoStorage::new_with_migration(
                    &format!("sqlite:{path}?mode=rwc"),
                    BlobStorageType::DB,
                )
                .or_else(|e| {
                    warn!(
                        "Failed to open storage, falling back to memory storage: {}",
                        e
                    );
                    AutoStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
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
        let is_offline = remote.is_empty();

        let workspace = rt.block_on(async { self.get_workspace(&workspace_id).await });

        match workspace {
            Ok(mut workspace) => {
                if is_offline {
                    let identifier = nanoid!();
                    rt.block_on(async {
                        self.join_broadcast(&mut workspace, identifier.clone())
                            .await;
                    });
                    // prevent rt from being dropped, which will cause dropping the broadcast channel
                    std::mem::forget(rt);
                } else {
                    start_client_sync(
                        rt,
                        Arc::new(self.clone()),
                        self.sync_state.clone(),
                        remote,
                        workspace_id,
                    );
                }

                Ok(Workspace { workspace })
            }
            Err(e) => Err(e),
        }
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
