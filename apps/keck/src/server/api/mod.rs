mod blocks;

use crate::sync::{init, SQLite};

use super::*;
use axum::{
    extract::{ws::Message, Json, Path},
    http::{header, StatusCode},
    response::IntoResponse,
    Router,
};
use dashmap::DashMap;
use jwst::Block;
use lib0::any::Any;
use serde_json::{to_string as json_to_string, Value as JsonValue};
use std::convert::TryFrom;
use tokio::sync::{mpsc::Sender, Mutex};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use yrs::{Doc, Map, Transaction};

pub struct Context {
    pub doc: DashMap<String, Mutex<Doc>>,
    pub channel: DashMap<(String, String), Sender<Message>>,
    pub db: SQLite,
}

impl Context {
    pub async fn new() -> Self {
        Context {
            doc: DashMap::new(),
            channel: DashMap::new(),
            db: init("updates").await.unwrap(),
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        blocks::get_workspace,
        blocks::get_block,
        blocks::set_block,
    ),
    tags((name = "Blocks", description = "Read and write remote blocks"))
)]
struct ApiDoc;

pub fn api_docs() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/*tail").url("/api-doc/openapi.json", ApiDoc::openapi())
}

pub fn api_handler() -> Router {
    Router::new()
        .route(
            "/block/:workspace/:block",
            get(blocks::get_block).post(blocks::set_block),
        )
        .route("/block/:workspace", get(blocks::get_workspace))
}
