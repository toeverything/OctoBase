use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::JoinHandle as StdJoinHandler,
};

use jwst_codec::{decode_update_with_guid, encode_update_as_message, Doc, DocMessage, SyncMessage, SyncMessageScanner};
use tokio::{runtime::Runtime, sync::mpsc::channel, task::JoinHandle as TokioJoinHandler};

use super::*;

// just for test
pub fn memory_connector(
    rt: Arc<Runtime>,
    doc: Doc,
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
    let id = rand::random::<usize>();

    //  recv thread
    let recv_handler = {
        debug!("init memory recv thread");
        let finish = Arc::new(AtomicBool::new(false));

        {
            let finish = finish.clone();
            doc.subscribe(move |update| {
                debug!("send change: {}", update.len());

                match encode_update_as_message(update.to_vec()) {
                    Ok(buffer) => {
                        if rt.block_on(remote_sender.send(buffer)).is_err() {
                            // pipeline was closed
                            finish.store(true, Ordering::Release);
                        }
                        debug!("send change: {} end", update.len());
                    }
                    Err(e) => {
                        error!("write sync message error: {}", e);
                    }
                }
            });
        }

        {
            let local_sender = local_sender.clone();
            let doc = doc.clone();
            std::thread::spawn(move || {
                while let Ok(false) | Err(false) = finish
                    .compare_exchange_weak(true, false, Ordering::Acquire, Ordering::Acquire)
                    .or_else(|_| Ok(local_sender.is_closed()))
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
                doc.unsubscribe_all();
                debug!("recv final: {}", id);
            })
        }
    };

    // send thread
    let send_handler = {
        debug!("init memory send thread");
        tokio::spawn(async move {
            while let Some(msg) = local_receiver.recv().await {
                match msg {
                    Message::Binary(data) => {
                        info!("msg:binary");
                        let mut doc = doc.clone();
                        tokio::task::spawn_blocking(move || {
                            trace!("recv change: {:?}", data.len());
                            for update in SyncMessageScanner::new(&data).filter_map(|m| {
                                m.ok().and_then(|m| {
                                    if let SyncMessage::Doc(DocMessage::Update(update)) = m {
                                        Some(update)
                                    } else {
                                        None
                                    }
                                })
                            }) {
                                match decode_update_with_guid(update.clone()) {
                                    Ok((_, update1)) => {
                                        if let Err(e) = doc.apply_update_from_binary(update1) {
                                            error!("failed to decode update1: {}, update: {:?}", e, update);
                                        }
                                    }
                                    Err(e) => {
                                        error!("failed to decode update2: {}, update: {:?}", e, update);
                                    }
                                }
                            }

                            trace!("recv change: {} end", data.len());
                        });
                    }
                    Message::Close => {
                        info!("msg:close");
                        break;
                    }
                    Message::Ping => {
                        info!("msg:ping");
                        continue;
                    }
                }
            }
            debug!("send final: {}", id);
        })
    };

    (local_sender, remote_receiver, send_handler, recv_handler)
}
