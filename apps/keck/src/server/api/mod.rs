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
            webhook: Arc::default(),
        }
    }

    fn register_webhook(&self, workspace: Workspace) -> Workspace {
        if workspace.subscribe_count() == 0 {
            let client = reqwest::Client::new();
            let rt = tokio::runtime::Handle::current();
            let webhook = self.webhook.clone();
            workspace.subscribe_doc(move |_, history| {
                let webhook = webhook.read().unwrap();
                if webhook.is_empty() {
                    return;
                }
                rt.block_on(async {
                    debug!("send {} histories to webhook {}", history.len(), webhook);
                    let resp = client.post(webhook.as_str()).json(history).send().await.unwrap();
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
