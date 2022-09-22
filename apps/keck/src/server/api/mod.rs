mod blocks;

use crate::sync::init_pool;

use super::*;
use axum::{
    extract::{ws::Message, Json, Path},
    http::StatusCode,
    response::IntoResponse,
    Router,
};
use dashmap::DashMap;
use jwst::Block;
use serde_json::Value as JsonValue;
use sqlx::SqlitePool;
use std::convert::TryFrom;
use tokio::sync::{mpsc::Sender, Mutex};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use yrs::{Doc, Map, Subscription, Transaction, UpdateEvent};

pub struct BlockSubscription(Mutex<Subscription<UpdateEvent>>);

unsafe impl Send for BlockSubscription {}
unsafe impl Sync for BlockSubscription {}

impl From<Subscription<UpdateEvent>> for BlockSubscription {
    fn from(s: Subscription<UpdateEvent>) -> Self {
        Self(Mutex::new(s))
    }
}

pub struct Context {
    pub doc: DashMap<String, Mutex<Doc>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub db_conn: SqlitePool,
    pub subscribes: DashMap<String, BlockSubscription>,
}

impl Context {
    pub async fn new() -> Self {
        Context {
            doc: DashMap::new(),
            channel: DashMap::new(),
            db_conn: init_pool("jwst").await.unwrap(),
            subscribes: DashMap::new(),
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        blocks::get_workspace,
        blocks::set_workspace,
        blocks::get_block,
        blocks::set_block,
        blocks::insert_block,
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
        .route(
            "/block/:workspace",
            get(blocks::get_workspace)
                .post(blocks::set_workspace)
                .delete(blocks::delete_workspace),
        )
}
