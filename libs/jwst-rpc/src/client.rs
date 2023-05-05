use super::{types::JwstRpcResult, *};
use futures::{SinkExt, StreamExt};
use jwst::{warn, DocStorage, Workspace};
use jwst_storage::{JwstStorage, JwstStorageResult};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
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

async fn prepare_connection(remote: &str) -> JwstRpcResult<Socket> {
    debug!("generate remote config");
    let uri = Url::parse(remote)?;

    let mut req = uri.into_client_request()?;
    req.headers_mut()
        .append("Sec-WebSocket-Protocol", HeaderValue::from_static("AFFiNE"));

    debug!("connect to remote: {}", req.uri());
    Ok(connect_async(req).await?.0)
}

async fn init_connection(workspace: &Workspace, remote: &str) -> JwstRpcResult<Socket> {
    let mut socket = prepare_connection(remote).await?;

    debug!("create init message");
    let init_data = workspace.sync_init_message().await?;

    debug!("send init message");
    socket.send(Message::Binary(init_data)).await?;

    Ok(socket)
}

async fn join_sync_thread(
    first_sync: Arc<AtomicBool>,
    sync_state: Option<Arc<RwLock<SyncState>>>,
    workspace: &Workspace,
    socket: Socket,
    rx: &mut Receiver<Vec<u8>>,
    sender: &Sender<()>,
) -> JwstRpcResult<bool> {
    let (mut socket_tx, mut socket_rx) = socket.split();

    let id = workspace.id();
    let mut workspace = workspace.clone();
    debug!("start sync thread {id}");
    let mut should_run_full_migrate = true;
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
                            if should_run_full_migrate {
                                sender.send(()).await.unwrap();
                                should_run_full_migrate = false;
                            }
                            let buffer = workspace.sync_decode_message(&msg).await;
                            first_sync.store(true, Ordering::Release);
                            if let Some(state) = sync_state.clone() {
                                let mut state = state.write().await;
                                match *state {
                                    SyncState::Offline => { *state = SyncState::Initialized },
                                    SyncState::Initialized => { *state = SyncState::Syncing },
                                    SyncState::Error(_) => { *state = SyncState::Syncing },
                                    _ => {}
                                }
                            }
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
    sync_state: Option<Arc<RwLock<SyncState>>>,
    workspace: &Workspace,
    remote: String,
    rx: &mut Receiver<Vec<u8>>,
    sender: &Sender<()>,
) -> JwstRpcResult<bool> {
    let socket = init_connection(workspace, &remote).await?;
    join_sync_thread(first_sync, sync_state, workspace, socket, rx, sender).await
}

/// Responsible for
/// 1. sending local workspace modifications to the 'remote' end through a socket. It is
/// necessary to manually listen for changes in the workspace using workspace.observe(), and
/// manually trigger the 'tx.send()'.
/// 2. synchronizing 'remote' modifications from the 'remote' to the local workspace, and
/// encoding the updates before sending them back to the 'remote'
pub fn start_sync_thread(
    workspace: &Workspace,
    remote: String,
    mut rx: Receiver<Vec<u8>>,
    sync_state: Option<Arc<RwLock<SyncState>>>,
    rt: Arc<Runtime>,
    sender: Sender<()>,
) {
    debug!("spawn sync thread");
    let first_sync = Arc::new(AtomicBool::new(false));
    let first_sync_cloned = first_sync.clone();
    let workspace = workspace.clone();
    std::thread::spawn(move || {
        rt.block_on(async move {
            if !workspace.is_empty() {
                info!("Workspace not empty, starting async remote connection");
                let first_sync_cloned_2 = first_sync_cloned.clone();
                tokio::spawn(async move {
                    sleep(Duration::from_secs(2)).await;
                    first_sync_cloned_2.store(true, Ordering::Release);
                });
            } else {
                info!("Workspace empty, starting sync remote connection");
            }
            loop {
                match run_sync(
                    first_sync_cloned.clone(),
                    sync_state.clone(),
                    &workspace,
                    remote.clone(),
                    &mut rx,
                    &sender,
                )
                .await
                {
                    Ok(true) => {
                        debug!("sync thread finished");
                        first_sync_cloned.store(true, Ordering::Release);
                        if let Some(state) = sync_state.clone() {
                            let mut state = state.write().await;
                            *state = SyncState::Finished;
                        }
                        break;
                    }
                    Ok(false) => {
                        first_sync_cloned.store(true, Ordering::Release);
                        if let Some(state) = sync_state.clone() {
                            let mut state = state.write().await;
                            *state =
                                SyncState::Error("Remote sync connection disconnected".to_string());
                        }
                        warn!("Remote sync connection disconnected, try again in 2 seconds");
                        sleep(Duration::from_secs(3)).await;
                    }
                    Err(e) => {
                        if let Some(state) = sync_state.clone() {
                            let mut state = state.write().await;
                            *state = SyncState::Error("Remote sync error".to_string());
                        }
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

pub async fn get_workspace(
    storage: &JwstStorage,
    id: String,
) -> JwstStorageResult<(Workspace, Receiver<Vec<u8>>)> {
    let workspace = storage.docs().get(id.clone()).await?;
    // get the receiver corresponding to DocAutoStorage, the sender is used in the doc::write_update() method.
    let rx = match storage.docs().remote().write().await.entry(id.clone()) {
        Entry::Occupied(tx) => tx.get().subscribe(),
        Entry::Vacant(entry) => {
            let (tx, rx) = channel(100);
            entry.insert(tx);
            rx
        }
    };

    Ok((workspace, rx))
}
pub async fn get_collaborating_workspace(
    storage: &JwstStorage,
    id: String,
    remote: String,
    sender: Sender<()>,
) -> JwstStorageResult<Workspace> {
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

        start_sync_thread(
            &workspace,
            remote,
            rx,
            None,
            Arc::new(Runtime::new().unwrap()),
            sender,
        );
    }

    Ok(workspace)
}
