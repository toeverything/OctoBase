#[cfg(feature = "api")]
mod blobs;
#[cfg(feature = "api")]
mod blocks;

use super::{sync::CollaborationServer, *};
use axum::{extract::ws::Message, Router};
#[cfg(feature = "api")]
use axum::{
    extract::{Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, head},
};
use dashmap::DashMap;
use jwst::Workspace;
use jwst_storage::{BlobAutoStorage, DocAutoStorage};
use tokio::sync::{mpsc::Sender, Mutex};

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
    pub workspace: DashMap<String, Arc<Mutex<Workspace>>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub collaboration: CollaborationServer,
    pub docs: DocAutoStorage,
    pub blobs: BlobAutoStorage,
}

impl Context {
    pub async fn new(docs: Option<DocAutoStorage>, blobs: Option<BlobAutoStorage>) -> Self {
        Context {
            workspace: DashMap::new(),
            channel: DashMap::new(),
            collaboration: CollaborationServer::new().expect("Cannot init collaboration server"),
            docs: docs.unwrap_or(
                DocAutoStorage::init_sqlite_pool_with_name("jwst")
                    .await
                    .expect("Cannot create database"),
            ),
            blobs: blobs.unwrap_or(
                BlobAutoStorage::init_sqlite_pool_with_name("blobs")
                    .await
                    .expect("Cannot create database"),
            ),
        }
    }
}

pub fn api_handler(router: Router) -> Router {
    #[cfg(feature = "api")]
    {
        router.nest(
            "/api",
            blobs::blobs_apis(blocks::blocks_apis(Router::new())),
        )
    }
    #[cfg(not(feature = "api"))]
    {
        router
    }
}
