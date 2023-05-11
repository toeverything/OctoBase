use super::*;
use axum::{
    extract::{ws::WebSocketUpgrade, Path},
    response::Response,
    Json,
};
use jwst_rpc::{handle_connector, socket_connector};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub async fn auth_handler(Path(workspace_id): Path<String>) -> Json<WebSocketAuthentication> {
    info!("auth: {}", workspace_id);
    Json(WebSocketAuthentication {
        protocol: "AFFiNE".to_owned(),
    })
}

pub async fn upgrade_handler(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let identifier = nanoid!();
    if let Err(e) = context.create_workspace(workspace.clone()).await {
        error!("create workspace failed: {:?}", e);
    }
    ws.protocols(["AFFiNE"]).on_upgrade(move |socket| {
        handle_connector(context.clone(), workspace.clone(), identifier, move || {
            socket_connector(socket, &workspace)
        })
    })
}

#[cfg(test)]
mod test {
    use jwst::DocStorage;
    use jwst::{Block, Workspace};
    use jwst_logger::{error, info};
    use jwst_rpc::{get_workspace, start_sync_thread};
    use jwst_storage::JwstStorage;
    use libc::{kill, SIGTERM};
    use rand::{thread_rng, Rng};
    use std::collections::hash_map::Entry;
    use std::ffi::c_int;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::string::String;
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc::channel;

    #[test]
    #[ignore = "not needed in ci"]
    fn client_collaboration_with_server() {
        if dotenvy::var("KECK_DEBUG").is_ok() {
            jwst_logger::init_logger("keck");
        }
        let mut rng = thread_rng();
        let server_port = rng.gen_range(10000..=30000);
        let child = start_collaboration_server(server_port);

        let rt = Runtime::new().unwrap();
        let (workspace_id, mut workspace, storage) = rt.block_on(async move {
            let workspace_id = String::from("1");
            let storage: Arc<JwstStorage> = Arc::new(
                JwstStorage::new("sqlite::memory:")
                    .await
                    .expect("get storage: memory sqlite failed"),
            );
            let remote = String::from(format!("ws://localhost:{server_port}/collaboration/1"));
            storage
                .create_workspace(workspace_id.clone())
                .await
                .unwrap();

            if let Entry::Vacant(entry) = storage
                .docs()
                .remote()
                .write()
                .await
                .entry(workspace_id.clone())
            {
                let (tx, _rx) = tokio::sync::broadcast::channel(10);
                entry.insert(tx);
            };
            let (sender, _receiver) = channel::<()>(10);

            let (workspace, rx) = get_workspace(&storage, workspace_id.clone()).await.unwrap();
            if !remote.is_empty() {
                start_sync_thread(
                    &workspace,
                    remote,
                    rx,
                    None,
                    Arc::new(Runtime::new().unwrap()),
                    sender,
                );
            }

            (workspace_id, workspace, storage)
        });

        let workspace = {
            let id = workspace_id.clone();
            workspace.observe(move |_, e| {
                let id = id.clone();
                let rt = Runtime::new().unwrap();
                if let Err(e) =
                    rt.block_on(async { storage.docs().write_update(id, &e.update).await })
                {
                    error!("Failed to write update to storage: {:?}", e);
                }
            });

            workspace
        };

        for block_id in 0..3 {
            let block = create_block(&workspace, block_id.to_string(), "list".to_string());
            info!("from client, create a block: {:?}", block);
        }

        info!("------------------after sync------------------");

        for block_id in 0..3 {
            info!(
                "get block {block_id} from server: {}",
                get_block_from_server(workspace_id.clone(), block_id.to_string(), server_port)
            );
            assert!(!get_block_from_server(
                workspace_id.clone(),
                block_id.to_string(),
                server_port
            )
            .is_empty());
        }

        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
            let mut ids: Vec<_> = blocks.iter().map(|block| block.block_id()).collect();
            assert_eq!(ids.sort(), vec!["7", "8", "9"].sort());
            info!("blocks from local storage:");
            for block in blocks {
                info!("block: {:?}", block);
            }
        });

        close_collaboration_server(child);
    }

    #[test]
    #[ignore = "not needed in ci"]
    fn client_collaboration_with_server_with_poor_connection() {
        let mut rng = thread_rng();
        let server_port = rng.gen_range(30001..=65535);
        let child = start_collaboration_server(server_port);

        let rt = Runtime::new().unwrap();
        let workspace_id = String::from("1");
        let (storage, workspace) = rt.block_on(async {
            let storage: Arc<JwstStorage> = Arc::new(
                JwstStorage::new("sqlite::memory:")
                    .await
                    .expect("get storage: memory sqlite failed"),
            );
            let workspace = storage
                .docs()
                .get(workspace_id.clone())
                .await
                .expect("get workspace: {workspace_id} failed");
            (storage, workspace)
        });

        // simulate creating a block in offline environment
        let block = create_block(&workspace, "0".to_string(), "list".to_string());
        info!("from client, create a block: {:?}", block);
        info!(
            "get block 0 from server: {}",
            get_block_from_server(workspace_id.clone(), "0".to_string(), server_port)
        );
        assert!(
            get_block_from_server(workspace_id.clone(), "0".to_string(), server_port).is_empty()
        );

        let (workspace_id, mut workspace, storage) = rt.block_on(async move {
            let workspace_id = String::from("1");
            let remote = String::from(format!("ws://localhost:{server_port}/collaboration/1"));
            storage
                .create_workspace(workspace_id.clone())
                .await
                .unwrap();

            if let Entry::Vacant(entry) = storage
                .docs()
                .remote()
                .write()
                .await
                .entry(workspace_id.clone())
            {
                let (tx, _rx) = tokio::sync::broadcast::channel(10);
                entry.insert(tx);
            };

            let (workspace, rx) = get_workspace(&storage, workspace_id.clone()).await.unwrap();
            let (sender, _receiver) = channel::<()>(10);
            if !remote.is_empty() {
                start_sync_thread(
                    &workspace,
                    remote,
                    rx,
                    None,
                    Arc::new(Runtime::new().unwrap()),
                    sender,
                );
            }

            (workspace_id, workspace, storage)
        });

        let workspace = {
            let id = workspace_id.clone();
            let sub = workspace.observe(move |_, e| {
                let id = id.clone();
                let rt = Runtime::new().unwrap();
                if let Err(e) =
                    rt.block_on(async { storage.docs().write_update(id, &e.update).await })
                {
                    error!("Failed to write update to storage: {:?}", e);
                }
            });
            std::mem::forget(sub);

            workspace
        };

        info!("----------------start syncing from start_sync_thread()----------------");

        for block_id in 1..3 {
            let block = create_block(&workspace, block_id.to_string(), "list".to_string());
            info!("from client, create a block: {:?}", block);
            info!(
                "get block {block_id} from server: {}",
                get_block_from_server(workspace_id.clone(), block_id.to_string(), server_port)
            );
            assert!(!get_block_from_server(
                workspace_id.clone(),
                block_id.to_string(),
                server_port
            )
            .is_empty());
        }

        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
            let mut ids: Vec<_> = blocks.iter().map(|block| block.block_id()).collect();
            assert_eq!(ids.sort(), vec!["0", "1", "2"].sort());
            info!("blocks from local storage:");
            for block in blocks {
                info!("block: {:?}", block);
            }
        });

        info!("------------------after sync------------------");

        for block_id in 0..3 {
            info!(
                "get block {block_id} from server: {}",
                get_block_from_server(workspace_id.clone(), block_id.to_string(), server_port)
            );
            assert!(!get_block_from_server(
                workspace_id.clone(),
                block_id.to_string(),
                server_port
            )
            .is_empty());
        }

        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
            let mut ids: Vec<_> = blocks.iter().map(|block| block.block_id()).collect();
            assert_eq!(ids.sort(), vec!["0", "1", "2"].sort());
            info!("blocks from local storage:");
            for block in blocks {
                info!("block: {:?}", block);
            }
        });

        close_collaboration_server(child);
    }

    fn get_block_from_server(workspace_id: String, block_id: String, server_port: u16) -> String {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
            let resp = client
                .get(format!(
                    "http://localhost:{server_port}/api/block/{}/{}",
                    workspace_id, block_id
                ))
                .send()
                .await
                .unwrap();
            resp.text().await.unwrap()
        })
    }

    fn create_block(workspace: &Workspace, block_id: String, block_flavour: String) -> Block {
        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            space
                .create(&mut trx.trx, block_id, block_flavour)
                .expect("failed to create block")
        })
    }

    fn start_collaboration_server(port: u16) -> Child {
        let mut child = Command::new("cargo")
            .args(&["run", "-p", "keck"])
            .env("KECK_PORT", port.to_string())
            .env("USE_MEMORY_SQLITE", "true")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to run command");

        if let Some(ref mut stdout) = child.stdout {
            let reader = BufReader::new(stdout);

            for line in reader.lines() {
                let line = line.expect("Failed to read line");
                info!("{}", line);

                if line.contains("listening on 0.0.0.0:") {
                    info!("Keck server started");
                    break;
                }
            }
        }

        child
    }

    fn close_collaboration_server(child: Child) {
        unsafe { kill(child.id() as c_int, SIGTERM) };
    }
}
