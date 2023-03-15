mod api;
mod context;
mod error_status;
mod files;
mod layer;
mod tracing;
mod utils;

use axum::{http::Method, Extension, Router, Server};
use jwst_logger::{error, info, info_span, init_logger, print_versions};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    init_logger();
    print_versions(env!("CARGO_PKG_VERSION"));

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
        ])
        // allow requests from specific origin
        .allow_origin([
            "http://localhost:8080".parse().unwrap(),
            "https://affine-preview.vercel.app".parse().unwrap(),
        ])
        .allow_headers(Any);

    let context = Arc::new(context::Context::new().await);

    let app = tracing::make_tracing_layer(files::static_files(
        Router::new()
            .nest(
                "/api",
                api::make_api_doc_route(
                    api::make_rest_route(context.clone()).nest("/sync", api::make_ws_route()),
                ),
            )
            .layer(Extension(context.clone()))
            .layer(cors),
    ));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);

    if let Err(e) = Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(utils::shutdown_signal())
        .await
    {
        error!("Server shutdown due to error: {}", e);
    }

    info!("Server shutdown complete");
}
