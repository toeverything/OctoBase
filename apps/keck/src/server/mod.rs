mod api;
mod collaboration;
mod files;
mod utils;

use crate::{server::api::Context, utils::*};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use http::Method;
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};

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

    info!("signal received, starting graceful shutdown");
}

pub async fn start_server() {
    let origins = [
        "http://localhost:4200".parse().unwrap(),
        "http://127.0.0.1:4200".parse().unwrap(),
        "http://localhost:3000".parse().unwrap(),
        "http://127.0.0.1:3000".parse().unwrap(),
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

    let context = Arc::new(Context::new().await);

    let app = collaboration::collaboration_handler(api::api_handler(Router::new()))
        .layer(cors)
        .layer(Extension(context.clone()))
        .nest("/docs", get(files::docs_handler))
        .fallback(get(files::index_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    context.db.close().await;

    info!("shutdown final");
}
