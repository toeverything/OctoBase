use super::*;
use anyhow::Context;
use futures::{SinkExt, StreamExt};
use jwst::{DocStorage, JwstResult, Workspace};
use jwst_storage::JwstStorage;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::broadcast::{channel, Receiver},
    time::sleep,
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
                            let buffer = workspace.sync_decode_message(&msg);
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
    let first_sync = Arc::new(AtomicBool::new(false));
    let first_sync_cloned = first_sync.clone();
    let workspace = workspace.clone();
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Runtime::new() else {
            return error!("Failed to create runtime");
        };
        rt.block_on(async move {
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
                        debug!("sync thread finished");
                        break;
                    }
                    Ok(false) => {
                        warn!("Remote sync connection disconnected, try again in 2 seconds");
                        sleep(Duration::from_secs(3)).await;
                    }
                    Err(e) => {
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

    if let Entry::Vacant(entry) = storage.docs().remote().write().await.entry(id.clone()) {
        let (tx, rx) = channel(100);

        start_sync_thread(&workspace, remote, rx);

        entry.insert(tx);
    }

    Ok(workspace)
}
