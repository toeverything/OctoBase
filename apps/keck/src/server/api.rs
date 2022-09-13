use super::collaboration::DOC_MAP;
use super::*;
use axum::{extract::Path, Router};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        structured_data_handler
    ),
    tags((name = "Keck", description = "Keck API"))
)]
struct ApiDoc;

pub fn api_docs() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui/*tail").url("/api-doc/openapi.json", ApiDoc::openapi())
}

pub fn api_handler() -> Router {
    Router::new().nest("/data/:workspace", get(structured_data_handler))
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
