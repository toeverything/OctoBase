use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use super::*;
use crate::server::WorkspaceChangedBlocks;

pub fn generate_ws_callback(
    workspace_changed_blocks: &Arc<RwLock<HashMap<String, WorkspaceChangedBlocks>>>,
    runtime: &Arc<Runtime>,
) -> Arc<Box<dyn Fn(String, Vec<String>) + Send + Sync>> {
    let workspace_changed_blocks = workspace_changed_blocks.clone();
    let runtime = runtime.clone();
    Arc::new(Box::new(move |workspace_id, block_ids| {
        let workspace_changed_blocks = workspace_changed_blocks.clone();
        runtime.spawn(async move {
            let mut write_guard = workspace_changed_blocks.write().await;
            write_guard
                .entry(workspace_id.clone())
                .or_insert(WorkspaceChangedBlocks::new(workspace_id.clone()))
                .insert_block_ids(block_ids.clone());
        });
    }))
}

pub fn start_handling_observed_blocks(
    runtime: Arc<Runtime>,
    workspace_changed_blocks: Arc<RwLock<HashMap<String, WorkspaceChangedBlocks>>>,
    hook_endpoint: Arc<RwLock<String>>,
    client: Arc<Client>,
) {
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
