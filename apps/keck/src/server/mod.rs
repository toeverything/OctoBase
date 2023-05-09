mod api;
mod files;
mod sync;
mod utils;

use axum::{http::Method, Extension, Router, Server};
use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;
use std::{net::SocketAddr, sync::Arc, thread};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use tokio::{runtime, signal};
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

fn generate_ws_callback(
    workspace_changed_blocks: Arc<RwLock<HashMap<String, WorkspaceChangedBlocks>>>,
    runtime: Arc<Runtime>,
) -> Box<dyn Fn(String, Vec<String>) + Send + Sync> {
    Box::new(move |workspace_id, block_ids| {
        let workspace_changed_blocks = workspace_changed_blocks.clone();
        runtime.spawn(async move {
            let mut write_guard = workspace_changed_blocks.write().await;
            write_guard
                .entry(workspace_id.clone())
                .or_insert(WorkspaceChangedBlocks::new(workspace_id.clone()))
                .insert_block_ids(block_ids.clone());
        });
    })
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

    let context = Arc::new(Context::new(None).await);
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
    {
        let runtime = runtime.clone();
        let workspace_changed_blocks = workspace_changed_blocks.clone();
        let hook_endpoint = hook_endpoint.clone();
        let client = client.clone();
        thread::spawn(move || {
            let runtime_cloned = runtime.clone();
            loop {
                let workspace_changed_blocks = workspace_changed_blocks.clone();
                let hook_endpoint = hook_endpoint.clone();
                let runtime_cloned = runtime_cloned.clone();
                let client = client.clone();
                runtime.spawn(async move {
                    let read_guard = workspace_changed_blocks.read().await;
                    if !read_guard.is_empty() {
                        let endpoint = hook_endpoint.read().await;
                        if !endpoint.is_empty() {
                            {
                                let workspace_changed_blocks_clone = read_guard.clone();
                                let post_body = workspace_changed_blocks_clone
                                    .into_values()
                                    .collect::<Vec<WorkspaceChangedBlocks>>();
                                let endpoint = endpoint.clone();
                                runtime_cloned.spawn(async move {
                                    let response = client
                                        .post(endpoint.to_string())
                                        .json(&post_body)
                                        .send()
                                        .await;
                                    match response {
                                        Ok(response) => info!(
                                            "notified hook endpoint, endpoint response status: {}",
                                            response.status()
                                        ),
                                        Err(e) => error!("Failed to send notify: {}", e),
                                    }
                                });
                            }
                        }
                        drop(read_guard);
                        let mut write_guard = workspace_changed_blocks.write().await;
                        info!("workspace_changed_blocks: {:?}", write_guard);
                        write_guard.clear();
                    }
                });
                sleep(Duration::from_millis(200));
            }
        });
    }
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
