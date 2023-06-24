use super::*;
use axum::{
    body::BoxBody,
    http::{Response, Uri},
    response::IntoResponse,
    routing::get,
};
use cloud_infra::{fetch_static_response, rust_embed, RustEmbed};

#[derive(RustEmbed)]
#[folder = "../homepage/out/"]
#[include = "*"]
#[exclude = "*.txt"]
#[exclude = "*.map"]
struct Frontend;

async fn frontend_handler(uri: Uri) -> Response<BoxBody> {
    fetch_static_response(uri.clone(), true, Some(Frontend::get))
        .await
        .into_response()
}

pub fn static_files(router: Router) -> Router {
    if cfg!(debug_assertions) {
        router
    } else {
        router.fallback_service(get(frontend_handler))
    }
}
