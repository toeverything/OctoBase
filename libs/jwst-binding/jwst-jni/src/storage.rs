use crate::Workspace;
use android_logger::Config;
use jwst::{error, DocStorage, JwstError, JwstResult};
use jwst_rpc::start_client;
use jwst_storage::JwstStorage as AutoStorage;
use log::LevelFilter;
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::RwLock};

#[derive(Clone)]
pub struct JwstStorage {
    storage: Option<Arc<RwLock<AutoStorage>>>,
    error: Option<String>,
}

impl JwstStorage {
    pub fn new(path: String) -> Self {
        android_logger::init_once(
            Config::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag("jwst"),
        );

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

    fn sync(&self, workspace_id: String, remote: String) -> JwstResult<Workspace> {
        if let Some(storage) = &self.storage {
            let rt = Runtime::new().unwrap();

            let mut workspace = rt.block_on(async move {
                let storage = storage.read().await;

                start_client(&storage, workspace_id, remote).await
            })?;

            let (sub, workspace) = {
                let id = workspace.id();
                let storage = self.storage.clone();
                let sub = workspace.observe(move |_, e| {
                    let id = id.clone();
                    if let Some(storage) = storage.clone() {
                        let rt = Runtime::new().unwrap();
                        log::info!("update: {:?}", &e.update);
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
