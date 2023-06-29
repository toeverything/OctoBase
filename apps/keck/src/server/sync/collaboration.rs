use super::*;
use axum::{
    extract,
    extract::{ws::WebSocketUpgrade, Path},
    response::Response,
    Json,
};
use futures::FutureExt;
use jwst_rpc::{
    axum_socket_connector, handle_connector, webrtc_datachannel_server_connector,
    RTCSessionDescription,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::channel;

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
            axum_socket_connector(socket, &workspace)
        })
        .map(|_| ())
    })
}

pub async fn webrtc_handler(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    extract::Json(offer): extract::Json<RTCSessionDescription>,
) -> Json<RTCSessionDescription> {
    let (answer, tx, rx, _) = webrtc_datachannel_server_connector(offer).await;

    let (first_init_tx, mut first_init_rx) = channel::<bool>(10);
    let workspace_id = workspace.to_owned();
    tokio::spawn(async move {
        if let Some(true) = first_init_rx.recv().await {
            info!("socket init success: {}", workspace_id);
        } else {
            error!("socket init failed: {}", workspace_id);
        }
    });

    let identifier = nanoid!();
    tokio::spawn(async move {
        handle_connector(context.clone(), workspace.clone(), identifier, move || {
            (tx, rx, first_init_tx)
        })
        .await;
    });

    Json(answer)
}

#[cfg(test)]
mod test {
    use jwst::DocStorage;
    use jwst::{Block, Workspace};
    use jwst_logger::info;
    use jwst_rpc::{
        start_client_sync, start_webrtc_client_sync, BroadcastChannels, RpcContextImpl,
    };
    use jwst_storage::{BlobStorageType, JwstStorage};
    use libc::{kill, SIGTERM};
    use rand::{thread_rng, Rng};
    use std::ffi::c_int;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::string::String;
    use std::sync::Arc;
    use tokio::runtime::Runtime;

    struct TestContext {
        storage: Arc<JwstStorage>,
        channel: Arc<BroadcastChannels>,
    }

    impl TestContext {
        fn new(storage: Arc<JwstStorage>) -> Self {
            Self {
                storage,
                channel: Arc::default(),
            }
        }
    }

    impl RpcContextImpl<'_> for TestContext {
        fn get_storage(&self) -> &JwstStorage {
            &self.storage
        }

        fn get_channel(&self) -> &BroadcastChannels {
            &self.channel
        }
    }
    #[test]
    #[ignore = "not needed in ci"]
    fn client_collaboration_with_webrtc_server() {
        if dotenvy::var("KECK_DEBUG").is_ok() {
            jwst_logger::init_logger("keck");
        }

        // TODO:
        // there has a weird question:
        // use new terminal run keck is pass
        // use `start_collaboration_server` funtion is high probability block

        //let server_port = 3000;
        let server_port = thread_rng().gen_range(10000..=30000);
        let child = start_collaboration_server(server_port);

        let rt = Runtime::new().unwrap();
        let (workspace_id, workspace) = rt.block_on(async move {
            let workspace_id = "1";
            let context = Arc::new(TestContext::new(Arc::new(
                JwstStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
                    .await
                    .expect("get storage: memory sqlite failed"),
            )));
            let remote = format!("http://localhost:{server_port}/webrtc-sdp/1");

            start_webrtc_client_sync(
                Arc::new(Runtime::new().unwrap()),
                context.clone(),
                Arc::default(),
                remote,
                workspace_id.to_owned(),
            );

            (
                workspace_id.to_owned(),
                context.get_workspace(workspace_id).await.unwrap(),
            )
        });

        for block_id in 0..3 {
            let block = create_block(&workspace, block_id.to_string(), "list".to_string());
            info!("from client, create a block: {:?}", block);
        }

        info!("------------------after sync------------------");
        std::thread::sleep(std::time::Duration::from_secs(1));

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
    fn client_collaboration_with_server() {
        if dotenvy::var("KECK_DEBUG").is_ok() {
            jwst_logger::init_logger("keck");
        }

        let server_port = thread_rng().gen_range(10000..=30000);
        let child = start_collaboration_server(server_port);

        let rt = Runtime::new().unwrap();
        let (workspace_id, workspace) = rt.block_on(async move {
            let workspace_id = "1";
            let context = Arc::new(TestContext::new(Arc::new(
                JwstStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
                    .await
                    .expect("get storage: memory sqlite failed"),
            )));
            let remote = format!("ws://localhost:{server_port}/collaboration/1");

            start_client_sync(
                Arc::new(Runtime::new().unwrap()),
                context.clone(),
                Arc::default(),
                remote,
                workspace_id.to_owned(),
            );

            (
                workspace_id.to_owned(),
                context.get_workspace(workspace_id).await.unwrap(),
            )
        });

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
        let server_port = thread_rng().gen_range(30001..=65535);
        let child = start_collaboration_server(server_port);

        let rt = Runtime::new().unwrap();
        let workspace_id = String::from("1");
        let (storage, workspace) = rt.block_on(async {
            let storage: Arc<JwstStorage> = Arc::new(
                JwstStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
                    .await
                    .expect("get storage: memory sqlite failed"),
            );
            let workspace = storage
                .docs()
                .get_or_create_workspace(workspace_id.clone())
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

        let rt = Runtime::new().unwrap();
        let (workspace_id, workspace) = rt.block_on(async move {
            let workspace_id = "1";
            let context = Arc::new(TestContext::new(storage));
            let remote = format!("ws://localhost:{server_port}/collaboration/1");

            start_client_sync(
                Arc::new(Runtime::new().unwrap()),
                context.clone(),
                Arc::default(),
                remote,
                workspace_id.to_owned(),
            );

            (
                workspace_id.to_owned(),
                context.get_workspace(workspace_id).await.unwrap(),
            )
        });

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
            .env("KECK_LOG", "INFO")
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
