use super::collaboration::DOC_MAP;
use super::*;
use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::IntoResponse,
    Router,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

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
pub async fn get_workspace(Path(workspace): Path<String>) -> String {
    info!("structured_data: {}", workspace);
    if let Some(doc) = DOC_MAP.get(&workspace) {
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
pub async fn get_block(Path(params): Path<(String, String)>) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("structured_block: {}, {}", workspace, block);
    if let Some(doc) = DOC_MAP.get(&workspace) {
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
    get,
    path = "/api/data/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    )
)]
pub async fn set_block(Path(params): Path<(String, String)>) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("structured_block: {}, {}", workspace, block);
    if let Some(doc) = DOC_MAP.get(&workspace) {
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
