use std::thread::JoinHandle as StdJoinHandler;

use jwst_codec::Doc;
use nanoid::nanoid;
use tokio::{runtime::Runtime, sync::mpsc::channel, task::JoinHandle as TokioJoinHandler};

use super::*;

pub async fn connect_memory_workspace(
    server: Arc<MinimumServerContext>,
    init_state: &[u8],
    id: &str,
) -> (
    Doc,
    Sender<Message>,
    TokioJoinHandler<()>,
    StdJoinHandler<()>,
    Arc<Runtime>,
) {
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

    let mut doc = Doc::default();
    doc.apply_update_from_binary(init_state.to_vec()).unwrap();

    let (tx, rx, tx_handler, rx_handler) = memory_connector(rt.clone(), doc.clone());
    {
        let (last_synced_tx, mut last_synced_rx) = channel::<i64>(128);
        let tx = tx.clone();
        let workspace_id = id.to_string();
        {
            let rt = rt.clone();
            std::thread::spawn(move || {
                rt.block_on(handle_connector(server, workspace_id, nanoid!(), move || {
                    (tx, rx, last_synced_tx)
                }));
            });
        }

        let success = last_synced_rx.recv().await;

        if success.unwrap_or(0) > 0 {
            info!("{id} first init success");
        } else {
            error!("{id} first init failed");
        }

        rt.spawn(async move {
            while let Some(last_synced) = last_synced_rx.recv().await {
                info!("last synced: {}", last_synced);
            }
        });
    }

    (doc, tx, tx_handler, rx_handler, rt)
}
