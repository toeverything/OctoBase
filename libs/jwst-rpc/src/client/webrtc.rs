use super::*;
use nanoid::nanoid;
use reqwest::Client;
use std::sync::RwLock;
use tokio::{runtime::Runtime, sync::mpsc::channel};

async fn webrtc_connection(remote: &str) -> (Sender<Message>, Receiver<Vec<u8>>) {
    warn!("webrtc_connection start");
    let (offer, pc, tx, rx, mut s) = webrtc_datachannel_client_begin().await;
    let client = Client::new();

    match client.post(remote).json(&offer).send().await {
        Ok(res) => {
            webrtc_datachannel_client_commit(res.json::<RTCSessionDescription>().await.unwrap(), pc).await;
            s.recv().await.ok();
            warn!("client already connected");
        }
        Err(e) => {
            error!("failed to http post: {}", e);
        }
    }

    (tx, rx)
}

pub fn start_webrtc_client_sync(
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
            let workspace = match context.get_workspace(&workspace_id).await {
                Ok(workspace) => workspace,
                Err(e) => {
                    error!("failed to create workspace: {:?}", e);
                    return;
                }
            };
            if !workspace.is_empty() {
                info!("Workspace not empty, starting async remote connection");
                last_synced_tx.send(Utc::now().timestamp_millis()).await.unwrap();
            } else {
                info!("Workspace empty, starting sync remote connection");
            }

            loop {
                let (tx, rx) = webrtc_connection(&remote).await;
                *sync_state.write().unwrap() = SyncState::Connected;

                let ret = {
                    let identifier = nanoid!();
                    let workspace_id = workspace_id.clone();
                    let last_synced_tx = last_synced_tx.clone();
                    handle_connector(context.clone(), workspace_id, identifier, move || {
                        (tx, rx, last_synced_tx.clone())
                    })
                    .await
                };

                {
                    last_synced_tx.send(0).await.unwrap();
                    let mut state = sync_state.write().unwrap();
                    if ret {
                        debug!("sync thread finished");
                        *state = SyncState::Finished;
                    } else {
                        *state = SyncState::Error("Remote sync connection disconnected".to_string());
                    }
                }

                warn!("remote sync connection disconnected, try again in 2 seconds");
                sleep(Duration::from_secs(2)).await;
            }
        });
    });

    let timeline = CachedLastSynced::default();
    timeline.add_receiver(rt, last_synced_rx);

    timeline
}
