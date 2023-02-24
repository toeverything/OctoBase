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
use jwst_storage::JwstStorage;
use tokio::sync::mpsc::Sender;

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
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub storage: JwstStorage,
}

impl Context {
    pub async fn new(storage: Option<JwstStorage>) -> Self {
        let storage = if let Some(storage) = storage {
            info!("use external storage instance: {}", storage.database());
            Ok(storage)
        } else if let Ok(database_url) = dotenvy::var("DATABASE_URL") {
            info!("use external database: {}", database_url);
            JwstStorage::new(&database_url).await
        } else {
            info!("use sqlite database: jwst.db");
            JwstStorage::new_with_sqlite("jwst").await
        }
        .expect("Cannot create database");

        Context {
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
