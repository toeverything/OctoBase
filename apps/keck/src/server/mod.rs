mod api;
mod files;
mod subscribe;
mod sync;
mod utils;

use api::Context;
use axum::{http::Method, Extension, Router, Server};
use jwst::Workspace;
use std::{
    collections::HashMap,
    thread::sleep,
    {net::SocketAddr, sync::Arc},
};
use tokio::{runtime, signal, sync::RwLock};
use tower_http::cors::{Any, CorsLayer};

pub use subscribe::*;
pub use utils::*;

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

type WorkspaceRetrievalCallback = Option<Arc<Box<dyn Fn(&Workspace) + Send + Sync>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceChangedBlocks {
    #[serde(rename(serialize = "workspaceId"))]
    pub workspace_id: String,
    #[serde(rename(serialize = "blockIds"))]
    pub block_ids: Vec<String>,
}

impl WorkspaceChangedBlocks {
    pub fn new(workspace_id: String) -> WorkspaceChangedBlocks {
        WorkspaceChangedBlocks {
            workspace_id,
            block_ids: Vec::new(),
        }
    }

    pub fn insert_block_ids(&mut self, mut updated_block_ids: Vec<String>) {
        self.block_ids.append(&mut updated_block_ids);
    }
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

    let client = Arc::new(reqwest::Client::builder().no_proxy().build().unwrap());
    let runtime = Arc::new(
        runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_time()
            .enable_io()
            .build()
            .expect("Failed to create runtime"),
    );
    let workspace_changed_blocks =
        Arc::new(RwLock::new(HashMap::<String, WorkspaceChangedBlocks>::new()));
    let hook_endpoint = Arc::new(RwLock::new(String::new()));
    let cb: WorkspaceRetrievalCallback = {
        let workspace_changed_blocks = workspace_changed_blocks.clone();
        let runtime = runtime.clone();
        Some(Arc::new(Box::new(move |workspace: &Workspace| {
            workspace.set_callback(generate_ws_callback(&workspace_changed_blocks, &runtime));
        })))
    };
    let context = Arc::new(Context::new(None, cb).await);

    start_handling_observed_blocks(
        runtime.clone(),
        workspace_changed_blocks.clone(),
        hook_endpoint.clone(),
        client.clone(),
    );

    let app = files::static_files(sync::sync_handler(api::api_handler(Router::new())))
        .layer(cors)
        .layer(Extension(context.clone()))
        .layer(Extension(client))
        .layer(Extension(runtime))
        .layer(Extension(workspace_changed_blocks))
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
