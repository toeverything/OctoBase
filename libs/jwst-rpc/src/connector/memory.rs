use super::*;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::JoinHandle as StdJoinHandler,
};
use tokio::{sync::mpsc::channel, task::JoinHandle as TokioJoinHandler};
use yrs::{
    updates::{
        decoder::{Decode, DecoderV1},
        encoder::Encode,
    },
    Doc, Transact, Update,
};

// just for test
pub fn memory_connector(
    doc: Doc,
    id: usize,
) -> (
    Sender<Message>,
    Receiver<Vec<u8>>,
    TokioJoinHandler<()>,
    StdJoinHandler<()>,
) {
    // recv from remote pipeline
    let (remote_sender, remote_receiver) = channel::<Vec<u8>>(512);
    // send to remote pipeline
    let (local_sender, mut local_receiver) = channel::<Message>(100);

    //  recv thread
    let recv_handler = {
        debug!("init memory recv thread");
        let doc = doc.clone();
        let finish = Arc::new(AtomicBool::new(false));
        let sub = {
            let finish = finish.clone();
            doc.observe_update_v1(move |_, e| {
                use y_sync::sync::{Message, SyncMessage};

                debug!("send change: {}", e.update.len());
                if futures::executor::block_on(
                    remote_sender.send(
                        Message::Sync(SyncMessage::Update(e.update.clone()))
                            .encode_v1()
                            .expect("failed to encode update"),
                    ),
                )
                .is_err()
                {
                    // pipeline was closed
                    finish.store(true, Ordering::Release);
                }
                debug!("send change: {} end", e.update.len());
            })
            .unwrap()
        };

        let local_sender = local_sender.clone();
        std::thread::spawn(move || {
            while let Ok(false) | Err(false) = finish
                .compare_exchange_weak(true, false, Ordering::Acquire, Ordering::Acquire)
                .or_else(|_| Ok(local_sender.is_closed()))
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            drop(sub);
            debug!("recv final: {}", id);
        })
    };

    // send thread
    let send_handler = {
        debug!("init memory send thread");
        tokio::spawn(async move {
            while let Some(msg) = local_receiver.recv().await {
                match msg {
                    Message::Binary(data) => {
                        use y_sync::sync::{Message, MessageReader, SyncMessage};
                        let doc = doc.clone();
                        tokio::task::spawn_blocking(move || {
                            trace!("recv change: {}", data.len());
                            let mut decoder = DecoderV1::from(data.as_slice());
                            for update in MessageReader::new(&mut decoder).filter_map(|m| {
                                m.ok().and_then(|m| {
                                    if let Message::Sync(SyncMessage::Update(update)) = m {
                                        Some(update)
                                    } else {
                                        None
                                    }
                                })
                            }) {
                                match Update::decode_v1(&update) {
                                    Ok(update) => loop {
                                        match doc.try_transact_mut() {
                                            Ok(mut t) => {
                                                t.apply_update(update);
                                                break;
                                            }
                                            Err(_) => {
                                                std::thread::sleep(Duration::from_millis(100));
                                            }
                                        }
                                    },
                                    Err(e) => error!("failed to decode update: {}", e),
                                }
                            }

                            trace!("recv change: {} end", data.len());
                        });
                    }
                    Message::Close => break,
                    Message::Ping => continue,
                }
            }
            debug!("send final: {}", id);
        })
    };

    (local_sender, remote_receiver, send_handler, recv_handler)
}
