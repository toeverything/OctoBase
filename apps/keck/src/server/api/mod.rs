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
use jwst::Block;
use serde_json::Value as JsonValue;
use std::convert::TryFrom;
use tokio::sync::{mpsc::Sender, Mutex};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use yrs::{Doc, Map, Transaction};

pub struct Context {
    pub doc: DashMap<String, Mutex<Doc>>,
    pub storage: DashMap<String, Sender<Migrate>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub db: DbPool,
}

impl Context {
    pub async fn new() -> Self {
        Context {
            doc: DashMap::new(),
            storage: DashMap::new(),
            channel: DashMap::new(),
            db: DbPool::new(init_pool("jwst").await.expect("Cannot create database!")),
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        blocks::get_workspace,
        blocks::set_workspace,
        blocks::delete_workspace,
        blocks::workspace_client,
        blocks::history_workspace_clients,
        blocks::history_workspace,
        blocks::get_block,
        blocks::set_block,
        blocks::delete_block,
        blocks::insert_block,
        blocks::remove_block,
    ),
    tags((name = "Blocks", description = "Read and write remote blocks"))
)]
struct ApiDoc;

pub fn api_docs() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/*tail").url("/api-doc/openapi.json", ApiDoc::openapi())
}

pub fn api_handler() -> Router {
    let block_operation = Router::new()
        .route("/insert", post(blocks::insert_block))
        .route("/remove", post(blocks::remove_block));

    Router::new()
        .nest("/block/:workspace/:block/", block_operation)
        .route(
            "/block/:workspace/:block",
            get(blocks::get_block)
                .post(blocks::set_block)
                .delete(blocks::delete_block),
        )
        .route("/block/:workspace/client", get(blocks::workspace_client))
        .route(
            "/block/:workspace/history",
            get(blocks::history_workspace_clients),
        )
        .route(
            "/block/:workspace/history/:client",
            get(blocks::history_workspace),
        )
        .route(
            "/block/:workspace",
            get(blocks::get_workspace)
                .post(blocks::set_workspace)
                .delete(blocks::delete_workspace),
        )
}
