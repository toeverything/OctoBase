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

#[cfg(any(debug_assertions, all(not(debug_assertions), feature = "docs")))]
#[derive(RustEmbed)]
#[folder = "../handbook/book"]
#[include = "*"]
#[exclude = "*.txt"]
#[exclude = "*.map"]
struct Docs;

async fn docs_handler(uri: Uri) -> Response<BoxBody> {
    info!("get docs {:?}", uri);
    fetch_static_response(uri.clone(), false, Some(Docs::get))
        .await
        .into_response()
}

pub fn static_files(router: Router) -> Router {
    let router = if cfg!(any(
        debug_assertions,
        all(not(debug_assertions), feature = "docs")
    )) {
        // Redirect to docs in production until we have a proper home page
        // FIXME: Notice that you can't test this locally because the debug_assertions are used to determine where the built file are too
        // So, after this redirects you locally, it will still be 404, since the files will be expected at a production path like `/app/book`.
        router.nest_service("/docs", get(docs_handler))
    } else {
        router
    };
    let router = if cfg!(all(
        // redirect docs if release & not affine target & docs enabled
        not(debug_assertions),
        not(feature = "affine"),
        feature = "docs"
    )) {
        router.route("/", get(|| async { Redirect::to("/docs/index.html") }))
    } else {
        router
    };
    router.fallback_service(get(frontend_handler))
}
