use std::collections::hash_map::Entry;
use crate::Workspace;
use jwst::{DocStorage, error, info, JwstError, JwstResult};
use jwst_rpc::start_client;
use jwst_storage::{JwstStorage as AutoStorage, JwstStorage};
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
