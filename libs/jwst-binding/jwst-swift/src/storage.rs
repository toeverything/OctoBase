use std::collections::hash_map::Entry;
use crate::Workspace;
use jwst::{DocStorage, error, info, JwstError, JwstResult};
use jwst_rpc::start_client;
use jwst_storage::{JwstStorage as AutoStorage};
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::RwLock};
use tokio::sync::broadcast::channel;

#[derive(Clone)]
pub struct Storage {
    pub(crate) storage: Option<Arc<RwLock<AutoStorage>>>,
    pub(crate) error: Option<String>,
}

impl Storage {
    pub fn new(path: String) -> Self {
        let rt = Runtime::new().unwrap();

        match rt.block_on(AutoStorage::new(&format!("sqlite:{path}?mode=rwc"))) {
            Ok(pool) => Self {
                storage: Some(Arc::new(RwLock::new(pool))),
                error: None,
            },
            Err(e) => Self {
                storage: None,
                error: Some(e.to_string()),
            },
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
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

            let mut workspace = rt.block_on(async move {
                let storage = storage.read().await;
                if let Entry::Vacant(entry) = storage.docs().remote().write().await.entry(workspace_id.clone()) {
                    let (tx, _rx) = channel(100);
                    entry.insert(tx);
                }
                start_client(&storage, workspace_id, remote).await
            })?;

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
    use tokio::runtime::Runtime;
    use jwst_storage::{JwstStorage as AutoStorage};
    use crate::Storage;

    #[tokio::test]
    async fn get_storage() {
        let storage = AutoStorage::new(&format!("sqlite::memory?mode=rwc")).await.unwrap();
        let workspace = storage.create_workspace("1").await.unwrap();
        assert_eq!(workspace.id(), "1");
    }

    #[test]
    #[ignore = "need manually start keck server"]
    fn collaboration_test() {
        let mut storage = Storage::new("memory".to_string());
        let workspace_id = "1";
        let block_id = "1";
        let workspace = storage.connect(workspace_id.to_string(), format!("ws://localhost:3000/collaboration/{workspace_id}").to_string()).unwrap();
        let block = workspace.create(block_id.to_string(), "list".to_string());
        let resp = get_block_from_server(workspace_id.to_string(), block.id().to_string());
        assert!(!resp.is_empty());
    }

    fn get_block_from_server(workspace_id: String, block_id: String) -> String {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
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