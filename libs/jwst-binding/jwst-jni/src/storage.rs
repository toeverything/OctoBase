use std::sync::{Arc, RwLock};

use android_logger::Config;
use futures::TryFutureExt;
use jwst_rpc::{start_websocket_client_sync, BroadcastChannels, CachedLastSynced, RpcContextImpl, SyncState};
use jwst_storage::{BlobStorageType, JwstStorage as AutoStorage, JwstStorageResult};
use nanoid::nanoid;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc::channel,
};
use yrs::{ReadTxn, StateVector, Transact};

use super::*;

#[derive(Clone)]
pub struct JwstStorage {
    storage: Arc<AutoStorage>,
    channel: Arc<BroadcastChannels>,
    error: Option<String>,
    sync_state: Arc<RwLock<SyncState>>,
    last_sync: CachedLastSynced,
    difflog: CachedDiffLog,
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
            difflog: CachedDiffLog::default(),
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
                .map_err(JwstError::Io)?,
        );
        let is_offline = remote.is_empty();

        let workspace = rt.block_on(async { self.get_workspace(&workspace_id).await });

        match workspace {
            Ok(mut workspace) => {
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

                let update = workspace
                    .doc()
                    .transact()
                    .encode_state_as_update_v1(&StateVector::default())
                    .unwrap();

                let jwst_workspace = match jwst_core::Workspace::from_binary(update, &workspace_id) {
                    Ok(mut ws) => {
                        info!(
                            "Successfully applied to jwst workspace, jwst blocks: {}, yrs blocks: {}",
                            ws.get_blocks().map(|s| s.block_count()).unwrap_or_default(),
                            workspace
                                .retry_with_trx(|mut t| t.get_blocks(), 50)
                                .map(|s| s.block_count())
                                .unwrap_or_default()
                        );

                        Some(ws)
                    }
                    Err(e) => {
                        error!("Failed to apply to jwst workspace: {:?}", e);
                        None
                    }
                };

                let (sender, receiver) = channel::<Log>(10240);
                self.difflog.add_receiver(receiver, rt.clone(), self.storage.clone());

                let mut ws = Workspace {
                    workspace,
                    jwst_workspace,
                    runtime: rt,
                    sender,
                };

                if let Some(ret) = Workspace::compare(&mut ws) {
                    info!("Run first compare at workspace init: {}, {}", workspace_id, ret);
                } else {
                    warn!(
                        "Failed to run first compare, jwst workspace not initialed: {}",
                        workspace_id
                    );
                }

                Ok(ws)
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
