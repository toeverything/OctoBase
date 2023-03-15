use axum::body::Body;
use axum::http::Request;
use axum::{http::Method, Extension, Router, Server};
use jwst_logger::{error, info, info_span, init_logger, print_versions};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::request_id::{
    MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer,
};
use tower_http::trace::TraceLayer;

mod api;
mod context;
mod error_status;
mod files;
mod layer;
mod utils;

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

    let app = files::static_files(
        Router::new()
            .nest(
                "/api",
                api::make_api_doc_route(
                    api::make_rest_route(context.clone()).nest("/sync", api::make_ws_route()),
                ),
            )
            .layer(Extension(context.clone()))
            .layer(
                TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                    let request_id = request
                        .extensions()
                        .get::<RequestId>()
                        .and_then(|id| id.header_value().to_str().ok())
                        .unwrap_or_default();
                    info_span!(
                        "HTTP",
                        http.method = %request.method(),
                        http.url = %request.uri(),
                        request_id = %request_id,
                    )
                }),
            )
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
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
