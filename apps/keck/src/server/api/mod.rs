#[cfg(feature = "api")]
mod blocks;

use crate::sync::{init_pool, DbPool};

use super::{utils::Migrate, *};
use axum::{
    extract::{ws::Message, Json, Path},
    http::StatusCode,
    response::IntoResponse,
    Router,
};
use dashmap::DashMap;
use jwst::{parse_history, parse_history_client, Workspace};
use serde_json::Value as JsonValue;
use tokio::sync::{mpsc::Sender, Mutex};

pub struct Context {
    pub workspace: DashMap<String, Mutex<Workspace>>,
    pub storage: DashMap<String, Sender<Migrate>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub db: DbPool,
}

impl Context {
    pub async fn new() -> Self {
        Context {
            workspace: DashMap::new(),
            storage: DashMap::new(),
            channel: DashMap::new(),
            db: DbPool::new(init_pool("jwst").await.expect("Cannot create database!")),
        }
    }
}

pub fn api_handler(router: Router) -> Router {
    #[cfg(feature = "api")]
    {
        blocks::blocks_apis(router)
    }
    #[cfg(not(feature = "api"))]
    {
        router
    }
}
