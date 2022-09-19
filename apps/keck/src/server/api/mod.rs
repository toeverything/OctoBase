use crate::sync::{init, SQLite};

use super::*;
use axum::{
    extract::{ws::Message, Json, Path},
    http::{header, StatusCode},
    response::IntoResponse,
    Router,
};
use dashmap::DashMap;
use lib0::any::Any;
use serde_json::{to_string as json_to_string, Value as JsonValue};
use std::{collections::HashMap, convert::TryFrom};
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
        get_workspace,
        get_block,
        create_block,
        set_block,
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
            get(get_block).post(create_block).put(set_block),
        )
        .route("/block/:workspace", get(get_workspace))
}

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data")
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> String {
    info!("get_workspace: {}", workspace);
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        trx.get_map("blocks").to_json().to_string()
    } else {
        "".to_owned()
    }
}

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    responses(
        (status = 200, description = "Get block"),
        (status = 404, description = "Workspace or block content not found"),
        (status = 500, description = "Block data error")
    )
)]
pub async fn get_block(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("get_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        if let Some(block) = trx
            .get_map("blocks")
            .get("content")
            .and_then(|b| b.to_ymap())
            .and_then(|b| b.get(&block))
        {
            if let Ok(data) = serde_json::to_string(&block.to_json()) {
                ([(header::CONTENT_TYPE, "application/json")], data).into_response()
            } else {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

fn set_value(block: &mut Map, trx: &mut Transaction, key: &str, value: &JsonValue) {
    match value {
        JsonValue::Bool(v) => {
            block.insert(trx, key.clone(), *v);
        }
        JsonValue::Null => {
            block.remove(trx, key);
        }
        JsonValue::Number(v) => {
            if let Some(v) = v.as_f64() {
                block.insert(trx, key.clone(), v);
            } else if let Some(v) = v.as_i64() {
                block.insert(trx, key.clone(), v);
            } else if let Some(v) = v.as_u64() {
                block.insert(trx, key.clone(), i64::try_from(v).unwrap_or(0));
            }
        }
        JsonValue::String(v) => {
            block.insert(trx, key.clone(), v.clone());
        }
        _ => {}
    }
}

#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    request_body(
        content = String,
        description = "json",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Block created"),
        (status = 404, description = "Workspace not found"),
        (status = 500, description = "Failed to create block")
    )
)]
pub async fn create_block(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<JsonValue>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("create_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        let mut trx = doc.value().lock().await.transact();
        if let Some(block) = trx
            .get_map("blocks")
            .get("content")
            .and_then(|b| b.to_ymap())
            .and_then(|b| {
                b.insert(&mut trx, block.as_str(), HashMap::<String, Any>::new());
                b.get(&block)
            })
            .and_then(|b| b.to_ymap())
        {
            if let (Some(block_content), Some(mut content)) = (
                payload.as_object(),
                block.get("content").and_then(|b| b.to_ymap()),
            ) {
                for (key, value) in block_content.iter() {
                    set_value(&mut content, &mut trx, key, value);
                }
            }
            if let Ok(data) = json_to_string(&block.to_json()) {
                ([(header::CONTENT_TYPE, "application/json")], data).into_response()
            } else {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[utoipa::path(
    put,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    request_body(
        content = String,
        description = "json",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Set block"),
        (status = 404, description = "Workspace or block content not found"),
        (status = 500, description = "Block data error")
    )
)]
pub async fn set_block(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<JsonValue>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("set_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        let mut trx = doc.value().lock().await.transact();
        if let Some(block) = trx
            .get_map("blocks")
            .get("content")
            .and_then(|b| b.to_ymap())
            .and_then(|b| b.get(&block))
            .and_then(|b| b.to_ymap())
        {
            if let (Some(block_content), Some(mut content)) = (
                payload.as_object(),
                block.get("content").and_then(|b| b.to_ymap()),
            ) {
                for (key, value) in block_content.iter() {
                    if let Some(origin) = content.get(key) {
                        match (origin.to_json(), value) {
                            (
                                Any::Null | Any::Undefined,
                                JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_),
                            ) => {
                                set_value(&mut content, &mut trx, key, value);
                            }
                            (Any::Bool(origin), JsonValue::Bool(new)) => {
                                if &origin != new {
                                    set_value(&mut content, &mut trx, key, value);
                                }
                            }
                            (Any::Number(origin), JsonValue::Number(new)) if new.is_f64() => {
                                if origin != new.as_f64().unwrap() {
                                    set_value(&mut content, &mut trx, key, value);
                                }
                            }
                            (Any::BigInt(origin), JsonValue::Number(new)) if new.is_i64() => {
                                if origin != new.as_i64().unwrap() {
                                    set_value(&mut content, &mut trx, key, value);
                                }
                            }
                            (Any::String(origin), JsonValue::String(new)) => {
                                if &origin.to_string() != new {
                                    set_value(&mut content, &mut trx, key, value);
                                }
                            }
                            _ => {
                                set_value(&mut content, &mut trx, key, value);
                            }
                        }
                    } else {
                        set_value(&mut content, &mut trx, key, value);
                    }
                }
            }
            if let Ok(data) = json_to_string(&block.to_json()) {
                ([(header::CONTENT_TYPE, "application/json")], data).into_response()
            } else {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
