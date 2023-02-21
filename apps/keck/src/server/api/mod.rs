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
        let storage = if let Some(storage) = storage {
            Ok(storage)
        } else if let Ok(database_url) = dotenvy::var("DATABASE_URL") {
            JwstStorage::new(&database_url).await
        } else {
            JwstStorage::new_with_sqlite("jwst").await
        }
        .expect("Cannot create database");

        Context {
            workspace: DashMap::new(),
            channel: DashMap::new(),
            storage,
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
