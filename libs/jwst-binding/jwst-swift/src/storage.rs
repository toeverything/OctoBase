use crate::Workspace;
use jwst::{error, info, DocStorage, JwstError, JwstResult};
use jwst_rpc::{get_workspace, start_sync_thread, SyncState};
use jwst_storage::JwstStorage as AutoStorage;
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::RwLock};
use jwst_logger::{init_logger_with_level, Level};

#[derive(Clone)]
pub struct Storage {
    pub(crate) storage: Option<Arc<RwLock<AutoStorage>>>,
    pub(crate) error: Option<String>,
    pub(crate) sync_state: Arc<RwLock<SyncState>>,
}

impl Storage {
    pub fn new(path: String) -> Self {
        Self::new_with_log_level(path, "info".to_string())
    }

    pub fn new_with_log_level(path: String, level: String) -> Self {
        let level = match level.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" => Level::WARN,
            "error" => Level::ERROR,
            _ => Level::DEBUG
        };
        init_logger_with_level(level);
        let rt = Runtime::new().unwrap();

        match rt.block_on(AutoStorage::new(&format!("sqlite:{path}?mode=rwc"))) {
            Ok(pool) => Self {
                storage: Some(Arc::new(RwLock::new(pool))),
                error: None,
                sync_state: Arc::new(RwLock::new(SyncState::Offline)),
            },
            Err(e) => Self {
                storage: None,
                error: Some(e.to_string()),
                sync_state: Arc::new(RwLock::new(SyncState::Offline)),
            },
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    pub fn is_offline(&self) -> bool {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let sync_state = self.sync_state.read().await;
            matches!(*sync_state, SyncState::Offline)
        })
    }

    pub fn is_initialized(&self) -> bool {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let sync_state = self.sync_state.read().await;
            matches!(*sync_state, SyncState::Initialized)
        })
    }

    pub fn is_syncing(&self) -> bool {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let sync_state = self.sync_state.read().await;
            matches!(*sync_state, SyncState::Syncing)
        })
    }

    pub fn is_finished(&self) -> bool {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let sync_state = self.sync_state.read().await;
            matches!(*sync_state, SyncState::Finished)
        })
    }

    pub fn is_error(&self) -> bool {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let sync_state = self.sync_state.read().await;
            matches!(*sync_state, SyncState::Error(_))
        })
    }

    pub fn get_sync_state(&self) -> String {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let sync_state = self.sync_state.read().await;
            match *sync_state {
                SyncState::Offline => "offline".to_string(),
                SyncState::Syncing => "syncing".to_string(),
                SyncState::Initialized => "initialized".to_string(),
                SyncState::Finished => "finished".to_string(),
                SyncState::Error(_) => "Error".to_string(),
            }
        })
    }

    pub fn connect(&mut self, workspace_id: String, remote: String) -> Option<Workspace> {
        match self.sync(workspace_id, remote) {
            Ok(workspace) => Some(workspace),
            Err(e) => {
                error!("Failed to connect to workspace: {}", e);
                self.error = Some(e.to_string());
                None
            }
        }
    }

    pub fn sync(&self, workspace_id: String, remote: String) -> JwstResult<Workspace> {
        if let Some(storage) = &self.storage {
            let rt = Runtime::new().unwrap();

            let (mut workspace, rx) = rt.block_on(async move {
                let storage = storage.read().await;
                get_workspace(&storage, workspace_id).await.unwrap()
            });

            if !remote.is_empty() {
                start_sync_thread(&workspace, remote, rx, Some(self.sync_state.clone()));
            }

            let (sub, workspace) = {
                let id = workspace.id();
                let storage = self.storage.clone();
                let sub = workspace.observe(move |_, e| {
                    let id = id.clone();
                    if let Some(storage) = storage.clone() {
                        let rt = Runtime::new().unwrap();
                        info!("update: {:?}", &e.update);
                        if let Err(e) = rt.block_on(async move {
                            let storage = storage.write().await;
                            storage.docs().write_update(id, &e.update).await
                        }) {
                            error!("Failed to write update to storage: {}", e);
                        }
                    }
                });

                (sub, workspace)
            };

            Ok(Workspace {
                workspace,
                _sub: sub,
            })
        } else {
            Err(JwstError::WorkspaceNotInitialized(workspace_id))
        }
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
        let workspace = get_workspace(workspace_id);
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

    fn get_workspace(workspace_id: &str) -> Workspace {
        let mut storage = Storage::new("memory".to_string());
        storage
            .connect(
                workspace_id.to_string(),
                format!("ws://localhost:3000/collaboration/{workspace_id}").to_string(),
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
