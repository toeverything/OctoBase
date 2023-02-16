#[cfg(feature = "api")]
mod blobs;
#[cfg(feature = "api")]
mod blocks;

use super::*;
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
use jwst_storage::JwstStorage;
use tokio::sync::{mpsc::Sender, RwLock};

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
    pub workspace: DashMap<String, Arc<RwLock<Workspace>>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub storage: JwstStorage,
}

impl Context {
    pub async fn new(storage: Option<JwstStorage>) -> Self {
        Context {
            workspace: DashMap::new(),
            channel: DashMap::new(),
            storage: storage.unwrap_or(
                JwstStorage::new_with_sqlite("jwst")
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
