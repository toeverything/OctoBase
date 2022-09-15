use crate::sync::{init, SQLite};

use super::*;
use axum::{
    extract::{ws::Message, Path},
    http::{header, StatusCode},
    response::IntoResponse,
    Router,
};
use dashmap::DashMap;
use tokio::sync::{mpsc::Sender, Mutex};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use yrs::Doc;

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
            db: init("updates").await.unwrap()
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_workspace,
        get_block,
        set_block,
    ),
    tags((name = "Keck", description = "Keck API"))
)]
struct ApiDoc;

pub fn api_docs() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/*tail").url("/api-doc/openapi.json", ApiDoc::openapi())
}

pub fn api_handler() -> Router {
    Router::new()
        .route("/data/:workspace/:block", get(get_block).post(set_block))
        .route("/data/:workspace", get(get_workspace))
}

#[utoipa::path(
    get,
    path = "/api/data/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> String {
    info!("structured_data: {}", workspace);
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
    path = "/api/data/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    )
)]
pub async fn get_block(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("structured_block: {}, {}", workspace, block);
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

#[utoipa::path(
    post,
    path = "/api/data/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    )
)]
pub async fn set_block(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("structured_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        let mut trx = doc.value().lock().await.transact();
        if let Some(block) = trx
            .get_map("blocks")
            .get("content")
            .and_then(|b| b.to_ymap())
            .and_then(|b| b.get(&block))
            .and_then(|b| b.to_ymap())
        {
            // block.get_map("content").

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
