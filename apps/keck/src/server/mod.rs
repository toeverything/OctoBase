mod api;
mod files;
mod sync;
mod utils;

use api::Context;
use axum::{response::Redirect, Extension, Router, Server};
use http::Method;
use std::{net::SocketAddr, sync::Arc};
use tokio::{signal, sync::Mutex};
use tower_http::cors::{Any, CorsLayer};
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

    let port = 1280;

    if let Ok(lock) = named_lock::NamedLock::create("keck") {
        if let Ok(_lock) = lock.try_lock() {
            let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
            info!("listening on {}", addr);

            context
                .collaboration
                .listen(port)
                .await
                .expect(&format!("Cannot listen on port {port}"));
            context
                .collaboration
                .add_workspace(Arc::new(Mutex::new(jwst::Workspace::new("test"))))
                .await
                .unwrap();

            tokio::select! {
                Err(e) = Server::bind(&addr)
                    .serve(app.into_make_service())
                    .with_graceful_shutdown(shutdown_signal()) => {
                    error!("Server shutdown due to error: {}", e);
                }
                Err(e) = context.collaboration.serve() => {
                    error!("Collaboration server shutdown due to error: {}", e);
                }
                else => {
                    info!("Server shutdown complete");
                }
            };

            // context.docs.close().await;
            // context.blobs.close().await;

            info!("Server shutdown complete");
            return;
        }
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    info!("listening on {}", addr);

    context
        .collaboration
        .connect(port)
        .await
        .expect(&format!("Cannot listen on port {port}"));

    tokio::select! {
        Err(e) = Server::bind(&addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_signal()) => {
            error!("Server shutdown due to error: {}", e);
        }
        Err(e) = context.collaboration.serve() => {
            error!("Collaboration server shutdown due to error: {}", e);
        }
        else => {
            info!("Server shutdown complete");
        }
    };

    // context.docs.close().await;
    // context.blobs.close().await;

    info!("Server shutdown complete");
    return;
}
