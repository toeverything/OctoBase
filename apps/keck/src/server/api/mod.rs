#[cfg(feature = "api")]
mod blocks;

use crate::sync::DbPool;

use super::*;
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
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub db: DbPool,
}

impl Context {
    pub async fn new(default_pool: Option<DbPool>) -> Self {
        Context {
            workspace: DashMap::new(),
            channel: DashMap::new(),
            db: default_pool.unwrap_or(
                DbPool::init_pool("jwst")
                    .await
                    .expect("Cannot create database"),
            ),
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
