use std::{sync::Arc, time::Instant};

use chrono::Utc;
use jwst_core::{debug, error, info, trace, warn};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};

use super::{BroadcastType, Message, RpcContextImpl};

pub async fn handle_connector(
    context: Arc<impl RpcContextImpl<'static> + Send + Sync + 'static>,
    workspace_id: String,
    identifier: String,
    get_channel: impl FnOnce() -> (Sender<Message>, Receiver<Vec<u8>>, Sender<i64>),
) -> bool {
    info!("{} collaborate with workspace {}", identifier, workspace_id);

    // An abstraction of the established socket connection. Use tx to broadcast and
    // rx to receive.
    let (tx, rx, last_synced) = get_channel();

    let mut ws = context
        .get_workspace(&workspace_id)
        .await
        .expect("failed to get workspace");

    // Continuously receive information from the remote socket, apply it to the
    // local workspace, and send the encoded updates back to the remote end
    // through the socket.
    context
        .apply_change(&workspace_id, &identifier, tx.clone(), rx, last_synced.clone())
        .await;

    // Both of broadcast_update and server_update are sent to the remote socket
    // through 'tx' The 'broadcast_update' is the receiver for updates to the
    // awareness and Doc of the local workspace. It uses channel, which is owned
    // by the server itself and is stored in the server's memory (not persisted)."
    let broadcast_tx = context
        .join_broadcast(&mut ws, identifier.clone(), last_synced.clone())
        .await;
    let mut broadcast_rx = broadcast_tx.subscribe();
    // Obtaining the receiver corresponding to DocAutoStorage in storage. The sender
    // is used in the doc::write_update(). The remote used is the one belonging
    // to DocAutoStorage and is owned by the server itself, stored in the
    // server's memory (not persisted).
    let mut server_rx = context.join_server_broadcast(&workspace_id).await;

    // Send initialization message.
    match ws.sync_init_message().await {
        Ok(init_data) => {
            debug!("send init data:{:?}", init_data);
            if tx.send(Message::Binary(init_data)).await.is_err() {
                warn!("failed to send init message: {}", identifier);
                // client disconnected
                if let Err(e) = tx.send(Message::Close).await {
                    error!("failed to send close event: {}", e);
                }
                last_synced.send(0).await.unwrap();
                return false;
            }
        }
        Err(e) => {
            warn!("failed to generate {} init message: {}", identifier, e);
            if let Err(e) = tx.send(Message::Close).await {
                error!("failed to send close event: {}", e);
            }
            last_synced.send(0).await.unwrap();
            return false;
        }
    }

    last_synced.send(Utc::now().timestamp_millis()).await.unwrap();

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
    info!("{} stop collaborate with workspace {}", identifier, workspace_id);
    true
}

#[cfg(test)]
mod test {
    use std::sync::atomic::{AtomicU64, Ordering};

    use indicatif::{MultiProgress, ProgressBar, ProgressIterator, ProgressStyle};
    use jwst_core::JwstResult;

    use super::{
        super::{connect_memory_workspace, MinimumServerContext},
        *,
    };
    #[cfg(feature = "webrtc")]
    use crate::{
        webrtc_datachannel_client_begin, webrtc_datachannel_client_commit, webrtc_datachannel_server_connector,
    };

    #[tokio::test]
    #[ignore = "skip in ci"]
    #[cfg(feature = "webrtc")]
    async fn webrtc_datachannel_connector_test() {
        let (offer, pc, tx1, mut rx1, _) = webrtc_datachannel_client_begin().await;
        let (answer, tx2, mut rx2, _) = webrtc_datachannel_server_connector(offer).await;
        webrtc_datachannel_client_commit(answer, pc).await;

        let data_a_1 = String::from("data_a_1");
        let data_a_2 = String::from("data_a_2");
        let data_a_3 = String::from("data_a_3");

        let data_b_1 = String::from("data_b_1");
        let data_b_2 = String::from("data_b_2");
        let data_b_3 = String::from("data_b_3");

        tx1.send(Message::Binary(data_a_1.clone().into_bytes())).await.unwrap();
        tx1.send(Message::Binary(data_a_2.clone().into_bytes())).await.unwrap();

        tx2.send(Message::Binary(data_b_1.clone().into_bytes())).await.unwrap();
        tx2.send(Message::Binary(data_b_2.clone().into_bytes())).await.unwrap();

        tx1.send(Message::Binary(data_a_3.clone().into_bytes())).await.unwrap();
        tx2.send(Message::Binary(data_b_3.clone().into_bytes())).await.unwrap();

        if let Some(message) = rx2.recv().await {
            assert_eq!(String::from_utf8(message).ok().unwrap(), data_a_1);
        }
        if let Some(message) = rx2.recv().await {
            assert_eq!(String::from_utf8(message).ok().unwrap(), data_a_2);
        }

        if let Some(message) = rx1.recv().await {
            assert_eq!(String::from_utf8(message).ok().unwrap(), data_b_1);
        }
        if let Some(message) = rx1.recv().await {
            assert_eq!(String::from_utf8(message).ok().unwrap(), data_b_2);
        }

        if let Some(message) = rx2.recv().await {
            assert_eq!(String::from_utf8(message).ok().unwrap(), data_a_3);
        }
        if let Some(message) = rx1.recv().await {
            assert_eq!(String::from_utf8(message).ok().unwrap(), data_b_3);
        }
    }

    #[tokio::test]
    #[ignore = "unstable, skip in ci and wait for the refactoring of the sync logic"]
    async fn sync_test() -> JwstResult<()> {
        let workspace_id = format!("test{}", rand::random::<usize>());

        let (server, mut ws, init_state) = MinimumServerContext::new_with_workspace(&workspace_id).await;

        let (mut doc1, _, _, _, _) = connect_memory_workspace(server.clone(), &init_state, &workspace_id).await;
        let (doc2, tx2, tx_handler, rx_handler, rt) =
            connect_memory_workspace(server.clone(), &init_state, &workspace_id).await;

        // close connection after doc1 is broadcasted
        doc2.subscribe(move |_| {
            rt.block_on(async {
                tx2.send(Message::Close).await.unwrap();
            });
        });

        // collect the update from yrs's editing
        let update = {
            let mut ws = jwst_core::Workspace::from_binary(doc1.encode_update_v1().unwrap(), &workspace_id).unwrap();

            let mut space = ws.get_space("space").unwrap();
            let mut block1 = space.create("block1", "flavour1").unwrap();
            block1.set("key1", "val1").unwrap();

            ws.doc().encode_update_v1().unwrap()
        };
        // apply update with jwst-codec
        doc1.apply_update_from_binary(update).unwrap();

        // await the task to make sure the doc1 is broadcasted before check doc2
        tx_handler.await.unwrap();
        rx_handler.join().unwrap();

        // collect the update from jwst-codec and check the result
        {
            let mut ws = jwst_core::Workspace::from_binary(doc2.encode_update_v1().unwrap(), &workspace_id).unwrap();

            let space = ws.get_space("space").unwrap();
            let block1 = space.get("block1").unwrap();

            assert_eq!(block1.flavour(), "flavour1");
            assert_eq!(block1.get("key1").unwrap().to_string(), "val1");
        }

        {
            let space = ws.get_space("space").unwrap();
            let block1 = space.get("block1").unwrap();

            assert_eq!(block1.flavour(), "flavour1");
            assert_eq!(block1.get("key1").unwrap().to_string(), "val1");
        }

        Ok(())
    }

    #[ignore = "somewhat slow, only natively tested"]
    #[test]
    fn sync_test_cycle() -> JwstResult<()> {
        jwst_logger::init_logger("jwst-rpc");

        for _ in 0..1000 {
            sync_test()?;
        }
        Ok(())
    }

    async fn single_sync_stress_test(mp: &MultiProgress) -> JwstResult<()> {
        // jwst_logger::init_logger("jwst-rpc");

        let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .unwrap()
            .progress_chars("##-");

        let workspace_id = format!("test{}", rand::random::<usize>());

        let (server, mut ws, init_state) = MinimumServerContext::new_with_workspace(&workspace_id).await;

        let mut jobs = vec![];

        let collaborator_pb = mp.add(ProgressBar::new(10));
        collaborator_pb.set_style(style.clone());
        collaborator_pb.set_message("collaborators");
        let collaborator = Arc::new(AtomicU64::new(0));

        let pb = mp.add(ProgressBar::new(1000));
        pb.set_style(style.clone());
        pb.set_message("writing");
        for i in (0..1000).progress_with(pb) {
            let collaborator = collaborator.clone();
            let collaborator_pb = collaborator_pb.clone();
            let init_state = init_state.clone();
            let server = server.clone();
            let mut ws = ws.clone();

            collaborator.fetch_add(1, Ordering::Relaxed);
            collaborator_pb.set_position(collaborator.load(Ordering::Relaxed));
            let (doc, doc_tx, tx_handler, rx_handler, _rt) =
                connect_memory_workspace(server.clone(), &init_state, &workspace_id).await;
            let mut doc = jwst_core::Workspace::from_binary(doc.encode_update_v1().unwrap(), &workspace_id).unwrap();

            let handler = std::thread::spawn(move || {
                // close connection after doc1 is broadcasted
                let block_id = format!("block{}", i);
                {
                    let block_id = block_id.clone();
                    let doc_tx = doc_tx.clone();
                    doc.doc().subscribe(move |_u| {
                        // TODO: support changed block record
                        // let block_changed = t
                        //     .changed_parent_types()
                        //     .iter()
                        //     .filter_map(|ptr| {
                        //         let value: yrs::types::Value = (*ptr).into();
                        //         value.to_ymap()
                        //     })
                        //     .flat_map(|map| map.keys(t).map(|k| k.to_string()).collect::<Vec<_>>())
                        //     .any(|key| key == block_id);
                        let block_changed = false;

                        if block_changed {
                            if let Err(e) = futures::executor::block_on(doc_tx.send(Message::Close)) {
                                error!("send close message failed: {}", e);
                            }
                        }
                    });
                }

                {
                    let mut space = doc.get_space("space").unwrap();
                    let mut block = space.create(block_id.clone(), format!("flavour{}", i)).unwrap();
                    block.set(&format!("key{}", i), format!("val{}", i)).unwrap();
                }

                // await the task to make sure the doc1 is broadcasted before check doc2
                futures::executor::block_on(tx_handler).unwrap();
                rx_handler.join().unwrap();

                {
                    let space = ws.get_space("space").unwrap();
                    let block1 = space.get(format!("block{}", i)).unwrap();

                    assert_eq!(block1.flavour(), format!("flavour{}", i));
                    assert_eq!(
                        block1.get(&format!("key{}", i)).unwrap().to_string(),
                        format!("val{}", i)
                    );
                }

                collaborator.fetch_sub(1, Ordering::Relaxed);
                collaborator_pb.set_position(collaborator.load(Ordering::Relaxed));
            });
            jobs.push(handler);
        }

        let pb = mp.add(ProgressBar::new(jobs.len().try_into().unwrap()));
        pb.set_style(style.clone());
        pb.set_message("joining");
        for handler in jobs.into_iter().progress_with(pb) {
            if let Err(e) = handler.join() {
                panic!("{:?}", e.as_ref());
            }
        }

        info!("check the workspace");

        let pb = mp.add(ProgressBar::new(1000));
        pb.set_style(style.clone());
        pb.set_message("final checking");

        {
            let space = ws.get_space("space").unwrap();
            for i in (0..1000).progress_with(pb) {
                let block1 = space.get(format!("block{}", i)).unwrap();

                assert_eq!(block1.flavour(), format!("flavour{}", i));
                assert_eq!(
                    block1.get(&format!("key{}", i)).unwrap().to_string(),
                    format!("val{}", i)
                );
            }
        }

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
        // jwst_logger::init_logger("jwst-rpc");

        let mp = MultiProgress::new();
        let style = ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
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
