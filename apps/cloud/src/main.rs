mod api;
mod application;
mod config;
mod context;
mod infrastructure;
mod layer;
mod utils;

use axum::{http::Method, Extension, Router, Server};
use jwst_logger::{error, info, info_span, init_logger};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[tokio::main]
async fn main() {
    // ignore load error when missing env file
    let _ = dotenvy::dotenv();
    init_logger(PKG_NAME);
    jwst::print_versions(PKG_NAME, env!("CARGO_PKG_VERSION"));

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

    let app = layer::make_tracing_layer(
        Router::new()
            .nest(
                "/api",
                api::make_api_doc_route(
                    api::make_rest_route(context.clone()).nest("/sync", api::make_ws_route()),
                ),
            )
            .layer(Extension(context.clone()))
            .layer(cors),
    );

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
