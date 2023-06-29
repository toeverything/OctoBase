use crate::Workspace;
use futures::TryFutureExt;
use jwst::{error, warn, JwstError};
use jwst_logger::init_logger_with;
use jwst_rpc::{start_client_sync, BroadcastChannels, RpcContextImpl, SyncState};
use jwst_storage::{BlobStorageType, JwstStorage as AutoStorage, JwstStorageResult};
use nanoid::nanoid;
use std::sync::{Arc, RwLock};
use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct Storage {
    storage: Arc<AutoStorage>,
    channel: Arc<BroadcastChannels>,
    error: Option<String>,
    sync_state: Arc<RwLock<SyncState>>,
}

impl Storage {
    pub fn new(path: String) -> Self {
        Self::new_with_log_level(path, "info".to_string())
    }

    pub fn new_with_log_level(path: String, level: String) -> Self {
        init_logger_with(&format!("{}={}", env!("CARGO_PKG_NAME"), level));
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

impl RpcContextImpl<'_> for Storage {
    fn get_storage(&self) -> &AutoStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use crate::{Storage, Workspace};
    use tokio::runtime::Runtime;

    #[test]
    #[ignore = "need manually start collaboration server"]
    fn collaboration_test() {
        let (workspace_id, block_id) = ("1", "1");
        let workspace = get_workspace(workspace_id, None);
        let block = workspace.create(block_id.to_string(), "list".to_string());
        block.set_bool("bool_prop".to_string(), true);
        block.set_float("float_prop".to_string(), 1.0);
        block.push_children(&workspace.create("2".to_string(), "list".to_string()));

        let resp = get_block_from_server(workspace_id.to_string(), block.id().to_string());
        assert!(!resp.is_empty());
        let prop_extractor =
            r#"("prop:bool_prop":true)|("prop:float_prop":1\.0)|("sys:children":\["2"\])"#;
        let re = regex::Regex::new(prop_extractor).unwrap();
        assert_eq!(re.find_iter(resp.as_str()).count(), 3);
    }

    fn get_workspace(workspace_id: &str, offline: Option<()>) -> Workspace {
        let mut storage = Storage::new("memory".to_string());
        storage
            .connect(
                workspace_id.to_string(),
                if offline.is_some() {
                    "".to_string()
                } else {
                    format!("ws://localhost:3000/collaboration/{workspace_id}").to_string()
                },
            )
            .unwrap()
    }

    fn get_block_from_server(workspace_id: String, block_id: String) -> String {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let resp = client
                .get(format!(
                    "http://localhost:3000/api/block/{}/{}",
                    workspace_id, block_id
                ))
                .send()
                .await
                .unwrap();
            resp.text().await.unwrap()
        })
    }
}
