mod api;
mod sync;
mod utils;

use std::{net::SocketAddr, sync::Arc};

use api::Context;
use axum::{http::Method, Extension, Router, Server};
use jwst_core::Workspace;
use tokio::{runtime, signal, sync::RwLock};
use tower_http::cors::{Any, CorsLayer};
pub use utils::*;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
}

pub async fn start_server() {
    let origins = [
        "http://localhost:4200".parse().unwrap(),
        "http://127.0.0.1:4200".parse().unwrap(),
        "http://localhost:3000".parse().unwrap(),
        "http://127.0.0.1:3000".parse().unwrap(),
        "http://localhost:5173".parse().unwrap(),
        "http://127.0.0.1:5173".parse().unwrap(),
    ];

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        // allow requests from any origin
        .allow_origin(origins)
        .allow_headers(Any);

    let client = Arc::new(reqwest::Client::builder().no_proxy().build().unwrap());
    let runtime = Arc::new(
        runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_time()
            .enable_io()
            .build()
            .expect("Failed to create runtime"),
    );
    let hook_endpoint = Arc::new(RwLock::new(dotenvy::var("HOOK_ENDPOINT").unwrap_or_default()));

    let context = Arc::new(Context::new(None).await);

    let app = sync::sync_handler(api::api_handler(Router::new()))
        .layer(cors)
        .layer(Extension(context.clone()))
        .layer(Extension(client))
        .layer(Extension(runtime))
        .layer(Extension(hook_endpoint));

    let addr = SocketAddr::from((
        [0, 0, 0, 0],
        dotenvy::var("KECK_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(3000),
    ));
    info!("listening on {}", addr);

    if let Err(e) = Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        error!("Server shutdown due to error: {}", e);
    }

    // context.docs.close().await;
    // context.blobs.close().await;

    info!("Server shutdown complete");
}
