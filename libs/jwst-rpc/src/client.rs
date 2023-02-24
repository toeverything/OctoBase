use super::*;
use dashmap::mapref::entry::Entry;
use futures::{SinkExt, StreamExt};
use jwst::{DocStorage, JwstResult, Workspace};
use jwst_storage::JwstStorage;
use std::sync::Arc;
use tokio::sync::{
    mpsc::{channel, Receiver},
    RwLock,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};
use url::Url;

fn start_sync_thread(workspace: Arc<RwLock<Workspace>>, remote: String, mut rx: Receiver<Message>) {
    debug!("spawn sync thread");
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Runtime::new() else {
            return error!("Failed to create runtime");
        };
        rt.block_on(async move {
            debug!("generate remote config");
            let uri = match Url::parse(&remote) {
                Ok(uri) => uri,
                Err(e) => {
                    return error!("Failed to parse remote url: {}", e);
                }
            };
            let mut req = match uri.into_client_request() {
                Ok(req)=> req,
                Err(e) => {
                    return error!("Failed to create client request: {}", e);
                }
            };
            req.headers_mut()
                .append("Sec-WebSocket-Protocol", HeaderValue::from_static("AFFiNE"));

            debug!("connect to remote: {}", req.uri());
            let (mut socket_tx, mut socket_rx) = match connect_async(req).await {
                Ok((socket, _)) => socket.split(),
                Err(e) => {
                    return error!("Failed to connect to remote: {}", e);
                }
            };


            debug!("sync init message");
            match workspace.read().await.sync_init_message() {
                Ok(init_data) => {
                    debug!("send init message");
                    if let Err(e) = socket_tx.send(Message::Binary(init_data)).await {
                        error!("Failed to send init message: {}", e);
                        return;
                    }
                }
                Err(e) => {
                    error!("Failed to create init message: {}", e);
                    return;
                }
            }

            let id = workspace.read().await.id();
            debug!("start sync thread {id}");
            loop {
                tokio::select! {
                    Some(msg) = socket_rx.next() => {
                        match msg {
                            Ok(msg) => {
                                if let Message::Binary(msg) = msg {
                                    trace!("get update from remote: {:?}", msg);
                                    let buffer = workspace.write().await.sync_decode_message(&msg);
                                    for update in buffer {
                                        // skip empty updates
                                        if update == [0, 2, 2, 0, 0] {
                                            continue;
                                        }
                                        trace!("send differential update to remote: {:?}", update);
                                        if let Err(e) = socket_tx.send(Message::binary(update)).await {
                                            warn!("send update to remote failed: {:?}", e);
                                            if let Err(e) = socket_tx.close().await {
                                                error!("close failed: {}", e);
                                            };
                                            break;
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                error!("remote closed: {e}");
                                break;
                            },
                        }
                    }
                    Some(msg) = rx.recv() => {
                        trace!("send update to remote: {:?}", msg);
                        if let Err(e) = socket_tx.send(msg).await {
                            warn!("send update to remote failed: {:?}", e);
                            if let Err(e) = socket_tx.close().await{
                                error!("close failed: {}", e);
                            }
                            break;
                        }
                    }
                }
            }
            debug!("end sync thread");
        });
    });
}

pub async fn start_client(
    storage: JwstStorage,
    id: String,
    remote: String,
) -> JwstResult<Arc<RwLock<Workspace>>> {
    let workspace = storage.docs().get(id.clone()).await?;

    if let Entry::Vacant(entry) = storage.docs().remote().entry(id.clone()) {
        let (tx, rx) = channel(100);

        start_sync_thread(workspace.clone(), remote, rx);

        entry.insert(tx);
    }

    Ok(workspace)
}
