use super::*;
use anyhow::Context;
use futures::{SinkExt, StreamExt};
use jwst::{DocStorage, JwstResult, Workspace};
use jwst_storage::JwstStorage;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::{
    net::TcpStream,
    sync::broadcast::{channel, Receiver},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    MaybeTlsStream, WebSocketStream,
};
use url::Url;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn prepare_connection(remote: &str) -> JwstResult<Socket> {
    debug!("generate remote config");
    let uri = Url::parse(remote).context("failed to parse remote url".to_string())?;

    let mut req = uri
        .into_client_request()
        .context("failed to create client request")?;
    req.headers_mut()
        .append("Sec-WebSocket-Protocol", HeaderValue::from_static("AFFiNE"));

    debug!("connect to remote: {}", req.uri());
    Ok(connect_async(req)
        .await
        .context("failed to init connect")?
        .0)
}

async fn init_connection(workspace: &Workspace, remote: &str) -> JwstResult<Socket> {
    let mut socket = prepare_connection(remote).await?;

    debug!("create init message");
    let init_data = workspace
        .sync_init_message()
        .await
        .context("failed to create init message")?;

    debug!("send init message");
    socket
        .send(Message::Binary(init_data))
        .await
        .context("failed to send init message")?;

    Ok(socket)
}

async fn join_sync_thread(
    first_sync: Arc<AtomicBool>,
    workspace: &Workspace,
    socket: Socket,
    rx: &mut Receiver<Vec<u8>>,
) -> JwstResult<bool> {
    let (mut socket_tx, mut socket_rx) = socket.split();

    let id = workspace.id();
    let mut workspace = workspace.clone();
    debug!("start sync thread {id}");
    let success = loop {
        tokio::select! {
            Some(msg) = socket_rx.next() => {
                match msg {
                    Ok(msg) => {
                        if let Message::Binary(msg) = msg {
                            debug!("get update from remote: {:?}", msg);
                            let mut success = true;
                            // skip empty updates
                            if msg == [0, 2, 2, 0, 0] {
                                continue;
                            }
                            let buffer = workspace.sync_decode_message(&msg).await;
                            first_sync.store(true, Ordering::Release);
                            for update in buffer {
                                debug!("send differential update to remote: {:?}", update);
                                if let Err(e) = socket_tx.send(Message::binary(update)).await {
                                    warn!("send differential update to remote failed: {:?}", e);
                                    if let Err(e) = socket_tx.close().await {
                                        error!("close failed: {}", e);
                                    };
                                    success = false;
                                    break
                                }
                            }
                            if !success {
                                break success
                            }
                        }
                    },
                    Err(e) => {
                        error!("remote closed: {e}");
                        break false
                    },
                }
            }
            Ok(msg) = rx.recv() => {
                debug!("send local update to remote: {:?}", msg);
                if let Err(e) = socket_tx.send(Message::Binary(msg)).await {
                    warn!("send local update to remote failed: {:?}", e);
                    if let Err(e) = socket_tx.close().await{
                        error!("close failed: {}", e);
                    }
                    break true
                }
            }
        }
    };
    debug!("end sync thread {id}");

    Ok(success)
}

async fn run_sync(
    first_sync: Arc<AtomicBool>,
    workspace: &Workspace,
    remote: String,
    rx: &mut Receiver<Vec<u8>>,
) -> JwstResult<bool> {
    let socket = init_connection(workspace, &remote).await?;
    join_sync_thread(first_sync, workspace, socket, rx).await
}

fn start_sync_thread(workspace: &Workspace, remote: String, mut rx: Receiver<Vec<u8>>) {
    debug!("spawn sync thread");
    println!("start_sync_thread, remote: {remote}");
    let first_sync = Arc::new(AtomicBool::new(false));
    let first_sync_cloned = first_sync.clone();
    let workspace = workspace.clone();
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Runtime::new() else {
            return error!("Failed to create runtime");
        };
        rt.block_on(async move {
            let first_sync_cloned_2 = first_sync_cloned.clone();
            tokio::spawn(async move {
                sleep(Duration::from_secs(2)).await;
                first_sync_cloned_2.store(true, Ordering::Release);
            });
            loop {
                match run_sync(
                    first_sync_cloned.clone(),
                    &workspace,
                    remote.clone(),
                    &mut rx,
                )
                    .await
                {
                    Ok(true) => {
                        println!("sync thread finished");
                        debug!("sync thread finished");
                        first_sync_cloned.store(true, Ordering::Release);
                        break;
                    }
                    Ok(false) => {
                        println!("Remote sync connection disconnected, try again in 2 seconds");
                        first_sync_cloned.store(true, Ordering::Release);
                        warn!("Remote sync connection disconnected, try again in 2 seconds");
                        sleep(Duration::from_secs(3)).await;
                    }
                    Err(e) => {
                        println!("Remote sync error, try again in 3 seconds");
                        first_sync_cloned.store(true, Ordering::Release);
                        warn!("Remote sync error, try again in 3 seconds: {}", e);
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }

            debug!("end sync thread");
        });
    });

    while let Ok(false) | Err(false) =
        first_sync.compare_exchange_weak(true, false, Ordering::Acquire, Ordering::Acquire)
    {
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub async fn start_client(
    storage: &JwstStorage,
    id: String,
    remote: String,
) -> JwstResult<Workspace> {
    let workspace = storage.docs().get(id.clone()).await?;

    if !remote.is_empty() {
        // get the receiver corresponding to DocAutoStorage, the sender is used in the doc::write_update() method.
        let rx = match storage.docs().remote().write().await.entry(id.clone()) {
            Entry::Occupied(tx) => tx.get().subscribe(),
            Entry::Vacant(entry) => {
                let (tx, rx) = channel(100);
                entry.insert(tx);
                rx
            }
        };

        // Responsible for
        // 1. sending local workspace modifications to the 'remote' end through a socket. It is
        // necessary to manually listen for changes in the workspace using workspace.observe(), and
        // manually trigger the 'tx.send()'.
        // 2. synchronizing 'remote' modifications from the 'remote' to the local workspace, and
        // encoding the updates before sending them back to the 'remote'
        start_sync_thread(&workspace, remote, rx);
    }

    Ok(workspace)
}

#[cfg(test)]
mod test {
    use crate::start_client;
    use jwst::DocStorage;
    use jwst::{Block, Workspace};
    use jwst_logger::error;
    use jwst_storage::JwstStorage;
    use reqwest::Response;
    use std::collections::hash_map::Entry;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::string::String;
    use std::sync::Arc;
    use std::thread::{self};
    use std::time::Duration;
    use tokio::runtime::Runtime;

    #[test]
    #[ignore = "need to start keck server first"]
    fn client_collaboration_with_server() {
        create_db_dir();

        let mut child = Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("keck")
            .spawn()
            .expect("Failed to execute 'cargo run --target keck'");

        thread::sleep(Duration::from_secs(10));

        let rt = Runtime::new().unwrap();
        let (workspace_id, mut workspace, storage) = rt.block_on(async move {
            let workspace_id = String::from("1");
            let storage: Arc<JwstStorage> =
                Arc::new(JwstStorage::new_with_sqlite("jwst_client").await.unwrap());
            let remote = String::from("ws://localhost:3000/collaboration/1");
            storage
                .create_workspace(workspace_id.clone())
                .await
                .unwrap();

            create_workspace_with_api(workspace_id.clone()).await;

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

            let workspace = start_client(&storage, workspace_id.clone(), remote)
                .await
                .unwrap();

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
                    error!("Failed to write update to storage: {}", e);
                    println!("Failed to write update to storage: {}", e);
                }
            });
            std::mem::forget(sub);

            workspace
        };

        for block_id in 0..3 {
            let block = create_block(&workspace, block_id.to_string(), "list".to_string());
            println!("from client, create a block: {:?}", block);
        }

        println!("------------------after sync------------------");

        for block_id in 0..3 {
            println!(
                "get block {block_id} from server: {}",
                get_block_with_api_sync(workspace_id.clone(), block_id.to_string())
            );
            assert!(!get_block_with_api_sync(workspace_id.clone(), block_id.to_string()).is_empty());
        }

        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
            let mut ids: Vec<_> = blocks.iter().map(|block| block.block_id()).collect();
            assert_eq!(ids.sort(), vec!["7", "8", "9"].sort());
            println!("blocks from local storage:");
            for block in blocks {
                println!("block: {:?}", block);
            }
        });

        child.kill().expect("failed to terminate the command");
        fs::remove_dir_all("./data").unwrap();
        child.wait().expect("command wasn't running");
    }

    #[test]
    #[ignore = "need to start keck server first"]
    fn client_collaboration_with_server_with_poor_connection() {
        create_db_dir();

        let mut child = Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("keck")
            .spawn()
            .expect("Failed to execute 'cargo run --target keck'");

        thread::sleep(Duration::from_secs(10));

        let rt = Runtime::new().unwrap();
        let workspace_id = String::from("1");
        let (storage, workspace) = rt.block_on(async {
            let storage: Arc<JwstStorage> =
                Arc::new(JwstStorage::new_with_sqlite("jwst_client").await.expect("get storage: jwst_client.db failed"));
            let workspace = storage.docs().get(workspace_id.clone()).await.expect("get workspace: {workspace_id} failed");
            (storage, workspace)
        });

        // simulate creating a block in offline environment
        let block = create_block(&workspace, "0".to_string(), "list".to_string());
        println!("from client, create a block: {:?}", block);
        println!(
            "get block 0 from server: {}",
            get_block_with_api_sync(workspace_id.clone(), "0".to_string())
        );
        assert!(get_block_with_api_sync(workspace_id.clone(), "0".to_string()).is_empty());

        let (workspace_id, mut workspace, storage) = rt.block_on(async move {
            let workspace_id = String::from("1");
            let remote = String::from("ws://localhost:3000/collaboration/1");
            storage
                .create_workspace(workspace_id.clone())
                .await
                .unwrap();

            create_workspace_with_api(workspace_id.clone()).await;

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

            let workspace = start_client(&storage, workspace_id.clone(), remote)
                .await
                .unwrap();

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
                    error!("Failed to write update to storage: {}", e);
                    println!("Failed to write update to storage: {}", e);
                }
            });
            std::mem::forget(sub);

            workspace
        };

        println!("----------------start syncing from start_sync_thread()----------------");

        for block_id in 1..3 {
            let block = create_block(&workspace, block_id.to_string(), "list".to_string());
            println!("from client, create a block: {:?}", block);
            println!(
                "get block {block_id} from server: {}",
                get_block_with_api_sync(workspace_id.clone(), block_id.to_string())
            );
            assert!(!get_block_with_api_sync(workspace_id.clone(), block_id.to_string()).is_empty());
        }

        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
            let mut ids: Vec<_> = blocks.iter().map(|block| block.block_id()).collect();
            assert_eq!(ids.sort(), vec!["0", "1", "2"].sort());
            println!("blocks from local storage:");
            for block in blocks {
                println!("block: {:?}", block);
            }
        });

        println!("------------------after sync------------------");

        for block_id in 0..3 {
            println!(
                "get block {block_id} from server: {}",
                get_block_with_api_sync(workspace_id.clone(), block_id.to_string())
            );
            assert!(!get_block_with_api_sync(workspace_id.clone(), block_id.to_string()).is_empty());
        }

        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            let blocks = space.get_blocks_by_flavour(&trx.trx, "list");
            let mut ids: Vec<_> = blocks.iter().map(|block| block.block_id()).collect();
            assert_eq!(ids.sort(), vec!["0", "1", "2"].sort());
            println!("blocks from local storage:");
            for block in blocks {
                println!("block: {:?}", block);
            }
        });

        child.kill().expect("Failed to terminate the command");
        fs::remove_dir_all("./data").unwrap();
        child.wait().expect("command wasn't running");
    }

    fn create_db_dir() {
        let dir_path = Path::new("./data");
        if dir_path.exists() {
            fs::remove_dir_all("./data").unwrap();
        }
        match fs::create_dir(&dir_path) {
            Ok(_) => println!("Directory created: {:?}", dir_path),
            Err(err) => eprintln!("Failed to create directory: {}", err),
        }
    }

    async fn create_workspace_with_api(workspace_id: String) {
        let client = reqwest::Client::new();
        client
            .post(format!("http://localhost:3000/api/block/{}", workspace_id))
            .send()
            .await
            .unwrap();
    }

    async fn get_block_with_api(workspace_id: String, block_id: String) -> Response {
        let client = reqwest::Client::new();

        client
            .get(format!(
                "http://localhost:3000/api/block/{}/{}",
                workspace_id, block_id
            ))
            .send()
            .await
            .unwrap()
    }

    fn get_block_with_api_sync(workspace_id: String, block_id: String) -> String {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let resp = get_block_with_api(workspace_id.clone(), block_id.to_string()).await;
            resp.text().await.unwrap()
        })
    }

    fn create_block(workspace: &Workspace, block_id: String, block_flavour: String) -> Block {
        workspace.with_trx(|mut trx| {
            let space = trx.get_space("blocks");
            space.create(&mut trx.trx, block_id, block_flavour)
        })
    }
}
