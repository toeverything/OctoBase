#[cfg(feature = "api")]
mod blobs;
#[cfg(feature = "api")]
mod blocks;
mod doc;

use std::collections::HashMap;

use axum::Router;
#[cfg(feature = "api")]
use axum::{
    extract::{Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, head, post},
};
use doc::doc_apis;
use jwst_rpc::{BroadcastChannels, RpcContextImpl};
use jwst_storage::{BlobStorageType, JwstStorage, JwstStorageResult};
use tokio::sync::RwLock;

use super::*;

#[derive(Deserialize)]
#[cfg_attr(feature = "api", derive(utoipa::IntoParams))]
pub struct Pagination {
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    usize::MAX
}

#[derive(Serialize)]
pub struct PageData<T> {
    total: usize,
    data: T,
}

pub struct Context {
    channel: BroadcastChannels,
    storage: JwstStorage,
    webhook: Arc<std::sync::RwLock<String>>,
}

impl Context {
    pub async fn new(storage: Option<JwstStorage>) -> Self {
        let blob_storage_type = BlobStorageType::DB;

        let storage = if let Some(storage) = storage {
            info!("use external storage instance: {}", storage.database());
            Ok(storage)
        } else if dotenvy::var("USE_MEMORY_SQLITE").is_ok() {
            info!("use memory sqlite database");
            JwstStorage::new_with_migration("sqlite::memory:", blob_storage_type).await
        } else if let Ok(database_url) = dotenvy::var("DATABASE_URL") {
            info!("use external database: {}", database_url);
            JwstStorage::new_with_migration(&database_url, blob_storage_type).await
        } else {
            info!("use sqlite database: jwst.db");
            JwstStorage::new_with_sqlite("jwst", blob_storage_type).await
        }
        .expect("Cannot create database");

        Context {
            channel: RwLock::new(HashMap::new()),
            storage,
            webhook: Arc::new(std::sync::RwLock::new(
                dotenvy::var("HOOK_ENDPOINT").unwrap_or_default(),
            )),
        }
    }

    fn register_webhook(&self, workspace: Workspace) -> Workspace {
        #[cfg(feature = "api")]
        if workspace.subscribe_count() == 0 {
            use blocks::BlockHistory;

            let client = reqwest::Client::new();
            let rt = tokio::runtime::Handle::current();
            let webhook = self.webhook.clone();
            let ws_id = workspace.id();
            workspace.subscribe_doc(move |_, history| {
                if history.is_empty() {
                    return;
                }
                let webhook = webhook.read().unwrap();
                if webhook.is_empty() {
                    return;
                }
                // release the lock before move webhook
                let webhook = webhook.clone();
                rt.block_on(async {
                    debug!("send {} histories to webhook {}", history.len(), webhook);
                    let resp = client
                        .post(webhook)
                        .json(
                            &history
                                .iter()
                                .map(|h| (ws_id.as_str(), h).into())
                                .collect::<Vec<BlockHistory>>(),
                        )
                        .send()
                        .await
                        .unwrap();
                    if !resp.status().is_success() {
                        error!("failed to send webhook: {}", resp.status());
                    }
                });
            });
        }
        workspace
    }

    pub fn set_webhook(&self, endpoint: String) {
        let mut write_guard = self.webhook.write().unwrap();
        *write_guard = endpoint;
    }

    pub async fn get_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        self.storage
            .get_workspace(workspace_id)
            .await
            .map(|w| self.register_webhook(w))
    }

    pub async fn init_workspace<S>(&self, workspace_id: S, data: Vec<u8>) -> JwstStorageResult
    where
        S: AsRef<str>,
    {
        self.storage.init_workspace(workspace_id, data).await
    }

    pub async fn export_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Vec<u8>>
    where
        S: AsRef<str>,
    {
        self.storage.export_workspace(workspace_id).await
    }

    pub async fn create_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        self.storage
            .create_workspace(workspace_id)
            .await
            .map(|w| self.register_webhook(w))
    }
}

impl RpcContextImpl<'_> for Context {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}

pub fn api_handler(router: Router) -> Router {
    #[cfg(feature = "api")]
    {
        router.nest("/api", blobs::blobs_apis(blocks::blocks_apis(Router::new())))
    }
    #[cfg(not(feature = "api"))]
    {
        router
    }
}
