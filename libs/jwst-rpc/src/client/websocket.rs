use super::{types::JwstRpcResult, *};
use nanoid::nanoid;
use std::sync::RwLock;
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

pub fn start_websocket_client_sync(
    rt: Arc<Runtime>,
    context: Arc<impl RpcContextImpl<'static> + Send + Sync + 'static>,
    sync_state: Arc<RwLock<SyncState>>,
    remote: String,
    workspace_id: String,
) -> CachedLastSynced {
    debug!("spawn sync thread");
    let (last_synced_tx, last_synced_rx) = channel::<i64>(128);

    let runtime = rt.clone();
    std::thread::spawn(move || {
        runtime.block_on(async move {
            println!("start sync thread");
            let workspace = match context.get_workspace(&workspace_id).await {
                Ok(workspace) => workspace,
                Err(e) => {
                    println!("failed to create workspace: {:?}", e);
                    return;
                }
            };
            if !workspace.is_empty() {
                println!("Workspace not empty, starting async remote connection");
                last_synced_tx
                    .send(Utc::now().timestamp_millis())
                    .await
                    .unwrap();
            } else {
                println!("Workspace empty, starting sync remote connection");
            }

            loop {
                let socket = match prepare_connection(&remote).await {
                    Ok(socket) => socket,
                    Err(e) => {
                        println!("Failed to connect to remote, try again in 2 seconds: {}", e);
                        sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                };
                *sync_state.write().unwrap() = SyncState::Connected;

                let ret = {
                    let identifier = nanoid!();
                    let workspace_id = workspace_id.clone();
                    let last_synced_tx = last_synced_tx.clone();
                    handle_connector(
                        context.clone(),
                        workspace_id.clone(),
                        identifier,
                        move || {
                            let (tx, rx) = tungstenite_socket_connector(socket, &workspace_id);
                            (tx, rx, last_synced_tx)
                        },
                    )
                    .await
                };

                {
                    last_synced_tx.send(0).await.unwrap();
                    let mut state = sync_state.write().unwrap();
                    if ret {
                        println!("sync thread finished");
                        *state = SyncState::Finished;
                    } else {
                        *state =
                            SyncState::Error("Remote sync connection disconnected".to_string());
                    }
                }

                println!("Remote sync connection disconnected, try again in 2 seconds");
                sleep(Duration::from_secs(2)).await;
            }
        });
    });

    let timeline = CachedLastSynced::new();
    timeline.add_receiver(rt, last_synced_rx);

    timeline
}
