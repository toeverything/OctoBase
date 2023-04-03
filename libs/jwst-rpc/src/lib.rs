mod broadcast;
mod client;
mod connector;
mod context;
mod utils;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use client::{get_collaborating_workspace, get_workspace, start_sync_thread};
pub use connector::{memory_connector, socket_connector};
pub use context::RpcContextImpl;
pub use utils::{connect_memory_workspace, MinimumServerContext};

use jwst::{debug, error, info, trace, warn};
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SyncState {
    Offline,
    Initialized,
    Syncing,
    Finished,
    Error(String),
}

#[derive(Debug)]
pub enum Message {
    Binary(Vec<u8>),
    Close,
    Ping,
}

pub async fn handle_connector(
    context: Arc<impl RpcContextImpl<'static> + Send + Sync + 'static>,
    workspace_id: String,
    identifier: String,
    get_channel: impl FnOnce() -> (Sender<Message>, Receiver<Vec<u8>>, Sender<bool>),
) {
    info!("{} collaborate with workspace {}", identifier, workspace_id);

    // An abstraction of the established socket connection. Use tx to broadcast and rx to receive.
    let (tx, rx, first_init) = get_channel();

    let mut ws = context
        .get_workspace(&workspace_id)
        .await
        .expect("failed to get workspace");

    // Continuously receive information from the remote socket, apply it to the local workspace, and
    // send the encoded updates back to the remote end through the socket.
    context
        .apply_change(&workspace_id, &identifier, tx.clone(), rx)
        .await;

    // Both of broadcast_update and server_update are sent to the remote socket through 'tx'
    // The 'broadcast_update' is the receiver for updates to the awareness and Doc of the local workspace.
    // It uses channel, which is owned by the server itself and is stored in the server's memory (not persisted)."
    let broadcast_tx = context.join_broadcast(&mut ws, identifier.clone()).await;
    let mut broadcast_rx = broadcast_tx.subscribe();
    // Obtaining the receiver corresponding to DocAutoStorage in storage. The sender is used in the
    // doc::write_update(). The remote used is the one belonging to DocAutoStorage and is owned by
    // the server itself, stored in the server's memory (not persisted).
    let mut server_rx = context.join_server_broadcast(&workspace_id).await;

    // Send initialization message.
    match ws.sync_init_message().await {
        Ok(init_data) => {
            if tx.send(Message::Binary(init_data)).await.is_err() {
                warn!("failed to send init message: {}", identifier);
                // client disconnected
                if let Err(e) = tx.send(Message::Close).await {
                    error!("failed to send close event: {}", e);
                }
                first_init.send(false).await.unwrap();
                return;
            }
        }
        Err(e) => {
            warn!("failed to generate {} init message: {}", identifier, e);
            if let Err(e) = tx.send(Message::Close).await {
                error!("failed to send close event: {}", e);
            }
            first_init.send(false).await.unwrap();
            return;
        }
    }

    first_init.send(true).await.unwrap();

    'sync: loop {
        tokio::select! {
            Ok(msg) = server_rx.recv()=> {
                let ts = Instant::now();
                trace!("recv from server update: {:?}", msg);
                if tx.send(Message::Binary(msg.clone())).await.is_err() {
                    // pipeline was closed
                    break 'sync;
                }
                if ts.elapsed().as_micros() > 100 {
                    debug!("process server update cost: {}ms", ts.elapsed().as_micros());
                }

            },
            Ok(msg) = broadcast_rx.recv()=> {
                let ts = Instant::now();
                match msg {
                    BroadcastType::BroadcastAwareness(data) => {
                        let ts = Instant::now();
                        trace!(
                            "recv awareness update from broadcast: {:?}bytes",
                            data.len()
                        );
                        if tx.send(Message::Binary(data.clone())).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!(
                                "process broadcast awareness cost: {}ms",
                                ts.elapsed().as_micros()
                            );
                        }
                    }
                    BroadcastType::BroadcastContent(data) => {
                        let ts = Instant::now();
                        trace!("recv content update from broadcast: {:?}bytes", data.len());
                        if tx.send(Message::Binary(data.clone())).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!(
                                "process broadcast content cost: {}ms",
                                ts.elapsed().as_micros()
                            );
                        }
                    }
                    BroadcastType::CloseUser(user) if user == identifier => {
                        let ts = Instant::now();
                        if tx.send(Message::Close).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!("process close user cost: {}ms", ts.elapsed().as_micros());
                        }

                        break;
                    }
                    BroadcastType::CloseAll => {
                        let ts = Instant::now();
                        if tx.send(Message::Close).await.is_err() {
                            // pipeline was closed
                            break 'sync;
                        }
                        if ts.elapsed().as_micros() > 100 {
                            debug!("process close all cost: {}ms", ts.elapsed().as_micros());
                        }

                        break 'sync;
                    }
                    _ => {}
                }

                if ts.elapsed().as_micros() > 100 {
                    debug!("process broadcast cost: {}ms", ts.elapsed().as_micros());
                }
            },
            _ = sleep(Duration::from_secs(1)) => {
                if tx.is_closed() || tx.send(Message::Ping).await.is_err() {
                    break 'sync;
                }
            }
        }
    }

    // make a final store
    context
        .get_storage()
        .full_migrate(workspace_id.clone(), None, false)
        .await;
    let _ = broadcast_tx.send(BroadcastType::CloseUser(identifier.clone()));
    info!(
        "{} stop collaborate with workspace {}",
        identifier, workspace_id
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use indicatif::{MultiProgress, ProgressBar, ProgressIterator, ProgressStyle};
    use jwst::JwstResult;
    use yrs::Map;

    #[tokio::test]
    async fn sync_test() -> JwstResult<()> {
        let workspace_id = format!("test{}", rand::random::<usize>());

        let (server, ws, init_state) =
            MinimumServerContext::new_with_workspace(&workspace_id).await;

        let (doc1, _, _, _) =
            connect_memory_workspace(server.clone(), &init_state, &workspace_id).await;
        let (mut doc2, tx2, tx_handler, rx_handler) =
            connect_memory_workspace(server.clone(), &init_state, &workspace_id).await;

        // close connection after doc1 is broadcasted
        doc2.observe(move |_, _| {
            futures::executor::block_on(async {
                tx2.send(Message::Close).await.unwrap();
            });
        })
        .await;

        doc1.with_trx(|mut t| {
            let space = t.get_space("space");
            let block1 = space.create(&mut t.trx, "block1", "flavour1").unwrap();
            block1.set(&mut t.trx, "key1", "val1").unwrap();
        });

        // await the task to make sure the doc1 is broadcasted before check doc2
        tx_handler.await.unwrap();
        rx_handler.join().unwrap();

        doc2.retry_with_trx(
            |mut t| {
                let space = t.get_space("space");
                let block1 = space.get(&mut t.trx, "block1").unwrap();

                assert_eq!(block1.flavour(&t.trx), "flavour1");
                assert_eq!(block1.get(&t.trx, "key1").unwrap().to_string(), "val1");
            },
            10,
        );

        ws.retry_with_trx(
            |mut t| {
                let space = t.get_space("space");
                let block1 = space.get(&mut t.trx, "block1").unwrap();

                assert_eq!(block1.flavour(&t.trx), "flavour1");
                assert_eq!(block1.get(&t.trx, "key1").unwrap().to_string(), "val1");
            },
            10,
        );

        Ok(())
    }

    #[ignore = "somewhat slow, only natively tested"]
    #[test]
    fn sync_test_cycle() -> JwstResult<()> {
        jwst_logger::init_logger();

        for _ in 0..1000 {
            sync_test()?;
        }
        Ok(())
    }

    async fn single_sync_stress_test(mp: &MultiProgress) -> JwstResult<()> {
        // jwst_logger::init_logger();
        let style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-");

        let workspace_id = format!("test{}", rand::random::<usize>());

        let (server, ws, init_state) =
            MinimumServerContext::new_with_workspace(&workspace_id).await;

        let mut jobs = vec![];

        let pb = mp.add(ProgressBar::new(1000));
        pb.set_style(style.clone());
        pb.set_message("writing");
        for i in (0..1000).progress_with(pb) {
            let init_state = init_state.clone();
            let server = server.clone();
            let ws = ws.clone();

            let (mut doc, doc_tx, tx_handler, rx_handler) =
                connect_memory_workspace(server.clone(), &init_state, &workspace_id).await;

            let handler = std::thread::spawn(move || {
                // close connection after doc1 is broadcasted
                let block_id = format!("block{}", i);
                {
                    let block_id = block_id.clone();
                    let doc_tx = doc_tx.clone();
                    futures::executor::block_on(doc.observe(move |t, _| {
                        let block_changed = t
                            .changed_parent_types()
                            .iter()
                            .filter_map(|ptr| {
                                let value: yrs::types::Value = (*ptr).into();
                                value.to_ymap()
                            })
                            .flat_map(|map| map.keys(t).map(|k| k.to_string()).collect::<Vec<_>>())
                            .any(|key| key == block_id);

                        if block_changed {
                            if let Err(e) = futures::executor::block_on(doc_tx.send(Message::Close))
                            {
                                error!("send close message failed: {}", e);
                            }
                        }
                    }));
                }

                doc.retry_with_trx(
                    |mut t| {
                        let space = t.get_space("space");
                        let block = space
                            .create(&mut t.trx, block_id.clone(), format!("flavour{}", i))
                            .unwrap();
                        block
                            .set(&mut t.trx, &format!("key{}", i), format!("val{}", i))
                            .unwrap();
                    },
                    50,
                );

                futures::executor::block_on(async move {
                    // await the task to make sure the doc1 is broadcasted before check doc2
                    tx_handler.await.unwrap();
                    rx_handler.join().unwrap();
                });

                ws.retry_with_trx(
                    |mut t| {
                        let space = t.get_space("space");
                        let block1 = space.get(&mut t.trx, format!("block{}", i)).unwrap();

                        assert_eq!(block1.flavour(&t.trx), format!("flavour{}", i));
                        assert_eq!(
                            block1
                                .get(&t.trx, &format!("key{}", i))
                                .unwrap()
                                .to_string(),
                            format!("val{}", i)
                        );
                    },
                    50,
                );
            });
            jobs.push(handler);
        }

        let pb = mp.add(ProgressBar::new(jobs.len().try_into().unwrap()));
        pb.set_style(style.clone());
        pb.set_message("joining");
        for handler in jobs.into_iter().progress_with(pb) {
            if let Err(e) = handler.join() {
                panic!("{:?}", e);
            }
        }

        info!("check the workspace");

        let pb = mp.add(ProgressBar::new(1000));
        pb.set_style(style.clone());
        pb.set_message("final checking");
        ws.with_trx(|mut t| {
            let space = t.get_space("space");
            for i in (0..1000).progress_with(pb) {
                let block1 = space.get(&mut t.trx, format!("block{}", i)).unwrap();

                assert_eq!(block1.flavour(&t.trx), format!("flavour{}", i));
                assert_eq!(
                    block1
                        .get(&t.trx, &format!("key{}", i))
                        .unwrap()
                        .to_string(),
                    format!("val{}", i)
                );
            }
        });

        Ok(())
    }

    #[ignore = "somewhat slow, only natively tested"]
    #[tokio::test(flavor = "multi_thread")]
    async fn sync_stress_test() -> JwstResult<()> {
        let mp = MultiProgress::new();
        single_sync_stress_test(&mp).await
    }

    #[ignore = "somewhat slow, only natively tested"]
    #[tokio::test(flavor = "multi_thread")]
    async fn sync_stress_test_cycle() -> JwstResult<()> {
        // jwst_logger::internal_init_logger_with_level(jwst_logger::Level::WARN);

        let mp = MultiProgress::new();
        let style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-");

        let pb = mp.add(ProgressBar::new(1000));
        pb.set_style(style.clone());
        pb.set_message("cycle stress test");
        for _ in (0..1000).progress_with(pb.clone()) {
            single_sync_stress_test(&mp).await?;
        }
        Ok(())
    }
}
