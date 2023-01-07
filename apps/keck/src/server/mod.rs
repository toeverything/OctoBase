mod api;
mod files;
mod storage;
mod sync;
mod utils;

use axum::{response::Redirect, Extension, Router, Server};
use http::Method;
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};

use api::Context;
use utils::*;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
        ])
        // allow requests from any origin
        .allow_origin(origins)
        .allow_headers(Any);

    let context = Arc::new(Context::new(None, None).await);

    let app = files::static_files(sync::sync_handler(api::api_handler(Router::new())))
        .layer(cors)
        .layer(Extension(context.clone()));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);

    if let Err(e) = Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        error!("Server shutdown due to error: {}", e);
    }

    context.docs.close().await;
    context.blobs.close().await;

    info!("Server shutdown complete");
}
