use super::*;
use axum::{
    body::BoxBody,
    http::{Response, Uri},
    response::IntoResponse,
    routing::get,
};
use jwst_static::{fetch_static_response, rust_embed, RustEmbed};

#[cfg(debug_assertions)]
#[derive(RustEmbed)]
// need point to frontend, for the convenience of development, we point it to handbook.
#[folder = "../handbook/book"]
// #[folder = "../frontend/dist/apps/jwst"]
// #[folder = "../../../AFFiNE/packages/app/out/"]
#[include = "*"]
#[exclude = "*.txt"]
#[exclude = "*.map"]
struct Frontend;

#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "../../dist/"]
#[include = "*"]
#[exclude = "*.txt"]
#[exclude = "*.map"]
struct Frontend;

async fn frontend_handler(uri: Uri) -> Response<BoxBody> {
    info!("get static {:?}", uri);
    fetch_static_response(uri.clone(), true, Some(Frontend::get))
        .await
        .into_response()
}

#[derive(RustEmbed)]
#[folder = "../handbook/book"]
#[include = "*"]
#[exclude = "*.txt"]
#[exclude = "*.map"]
struct JwstDocs;

async fn jwst_docs_handler(uri: Uri) -> Response<BoxBody> {
    info!("get doc {:?}", uri);
    fetch_static_response(uri.clone(), false, Some(JwstDocs::get))
        .await
        .into_response()
}

pub fn static_files(router: Router) -> Router {
    if cfg!(debug_assertions) || std::env::var("JWST_DEV").is_ok() {
        router.nest_service("/docs/", get(jwst_docs_handler))
    } else {
        router
    }
    .fallback_service(get(frontend_handler))
}
