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
        structured_data_handler,
        structured_block_handler,
    ),
    tags((name = "Keck", description = "Keck API"))
)]
struct ApiDoc;

pub fn api_docs() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/*tail").url("/api-doc/openapi.json", ApiDoc::openapi())
}

pub fn api_handler() -> Router {
    Router::new()
        .route("/data/:workspace/:block", get(structured_block_handler))
        .route("/data/:workspace", get(structured_data_handler))
}

#[utoipa::path(get, path = "/api/data/:workspace")]
pub async fn structured_data_handler(Path(workspace): Path<String>) -> String {
    info!("structured_data: {}", workspace);
    let docs = DOC_MAP.lock().await;
    if let Some(doc) = docs.get(&workspace) {
        let mut trx = doc.transact();
        trx.get_map("blocks").to_json().to_string()
    } else {
        "".to_owned()
    }
}

#[utoipa::path(get, path = "/api/data/:workspace/:block")]
pub async fn structured_block_handler(Path(params): Path<(String, String)>) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("structured_block: {}, {}", workspace, block);
    let docs = DOC_MAP.lock().await;
    if let Some(doc) = docs.get(&workspace) {
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
