use super::{types::JwstRpcResult, *};
use nanoid::nanoid;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    RwLock,
};
use tokio::{net::TcpStream, runtime::Runtime, sync::mpsc::channel};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue},
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

pub fn start_client_sync(
    rt: Arc<Runtime>,
    context: Arc<impl RpcContextImpl<'static> + Send + Sync + 'static>,
    sync_state: Arc<RwLock<SyncState>>,
    remote: String,
    workspace_id: String,
) {
    debug!("spawn sync thread");
    let first_sync = Arc::new(AtomicBool::new(false));
    let first_sync_cloned = first_sync.clone();
    std::thread::spawn(move || {
        rt.block_on(async move {
            let workspace = match context.get_workspace(&workspace_id).await {
                Ok(workspace) => workspace,
                Err(e) => {
                    error!("failed to create workspace: {:?}", e);
                    return;
                }
            };
            if !workspace.is_empty() {
                info!("Workspace not empty, starting async remote connection");
                first_sync_cloned.store(true, Ordering::Release);
            } else {
                info!("Workspace empty, starting sync remote connection");
            }

            loop {
                let identifier = nanoid!();
                let workspace_id = workspace_id.clone();
                let socket = prepare_connection(&remote).await.unwrap();
                let first_init_tx = {
                    let (first_init_tx, mut first_init_rx) = channel::<bool>(10);
                    let first_sync = first_sync_cloned.clone();
                    let sync_state = sync_state.clone();
                    tokio::spawn(async move {
                        if let Some(true) = first_init_rx.recv().await {
                            first_sync.store(true, Ordering::Release);
                            let mut state = sync_state.write().unwrap();
                            match *state {
                                SyncState::Offline => *state = SyncState::Initialized,
                                SyncState::Initialized | SyncState::Error(_) => {
                                    *state = SyncState::Syncing
                                }
                                _ => {}
                            }
                        }
                    });
                    first_init_tx
                };

                let ret = {
                    let id = workspace_id.clone();
                    handle_connector(context.clone(), workspace_id, identifier, move || {
                        let (tx, rx) = tungstenite_socket_connector(socket, &id);
                        (tx, rx, first_init_tx)
                    })
                    .await
                };

                {
                    first_sync_cloned.store(true, Ordering::Release);
                    let mut state = sync_state.write().unwrap();
                    if ret {
                        debug!("sync thread finished");
                        *state = SyncState::Finished;
                    } else {
                        *state =
                            SyncState::Error("Remote sync connection disconnected".to_string());
                    }
                }

                warn!("Remote sync connection disconnected, try again in 2 seconds");
                sleep(Duration::from_secs(3)).await;
            }
        });
    });

    while let Ok(false) | Err(false) =
        first_sync.compare_exchange_weak(true, false, Ordering::Acquire, Ordering::Acquire)
    {
        std::thread::sleep(Duration::from_millis(100));
    }
}
