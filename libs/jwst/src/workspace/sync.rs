use super::*;
use jwst_codec::{
    convert_awareness_update, convert_awareness_y_update, write_sync_message, DocMessage,
    SyncMessage, SyncMessageScanner,
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    thread::sleep,
    time::Duration,
};
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    ReadTxn, StateVector, Transact, Update,
};

impl Workspace {
    pub fn sync_migration(&self, mut retry: i32) -> JwstResult<Vec<u8>> {
        let trx = loop {
            match self.doc.try_transact() {
                Ok(trx) => break trx,
                Err(e) => {
                    if retry > 0 {
                        retry -= 1;
                        sleep(Duration::from_micros(10));
                    } else {
                        return Err(JwstError::DocTransaction(e.to_string()));
                    }
                }
            }
        };
        Ok(trx.encode_state_as_update_v1(&StateVector::default())?)
    }

    pub async fn sync_init_message(&self) -> JwstResult<Vec<u8>> {
        let (sv, awareness_update) = {
            let mut retry = 50;
            let trx = loop {
                if let Ok(trx) = self.doc.try_transact() {
                    break trx;
                } else if retry > 0 {
                    retry -= 1;
                    tokio::time::sleep(Duration::from_micros(10)).await;
                } else {
                    return Err(JwstError::SyncInitTransaction);
                }
            };
            let sv = trx.state_vector();
            let update = self.awareness.read().await.update()?;
            (sv, convert_awareness_update(update))
        };

        let mut buffer = Vec::new();
        write_sync_message(
            &mut buffer,
            &SyncMessage::Doc(DocMessage::Step1(sv.encode_v1()?)),
        )?;
        write_sync_message(&mut buffer, &awareness_update)?;

        Ok(buffer)
    }

    pub async fn sync_decode_message(&mut self, buffer: &[u8]) -> Vec<Vec<u8>> {
        let mut result = vec![];

        let (awareness_msg, content_msg): (Vec<_>, Vec<_>) =
            SyncMessageScanner::new(buffer).flatten().partition(|msg| {
                matches!(msg, SyncMessage::Awareness(_) | SyncMessage::AwarenessQuery)
            });

        if !awareness_msg.is_empty() {
            let mut awareness = self.awareness.write().await;
            for msg in awareness_msg {
                match msg {
                    SyncMessage::AwarenessQuery => {
                        if let Ok(update) = awareness.update() {
                            let mut buffer = Vec::new();
                            if let Err(e) =
                                write_sync_message(&mut buffer, &convert_awareness_update(update))
                            {
                                warn!("failed to encode awareness update: {:?}", e);
                            } else {
                                result.push(buffer);
                            }
                        }
                    }
                    SyncMessage::Awareness(update) => {
                        if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
                            awareness.apply_update(convert_awareness_y_update(update))
                        })) {
                            warn!("failed to apply awareness: {:?}", e);
                        }
                    }
                    _ => {}
                }
            }
        }
        if !content_msg.is_empty() {
            let doc = self.doc();
            if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
                let mut retry = 30;
                let mut trx = loop {
                    if let Ok(trx) = doc.try_transact_mut() {
                        break trx;
                    } else if retry > 0 {
                        retry -= 1;
                        sleep(Duration::from_micros(10));
                    } else {
                        return;
                    }
                };
                for msg in content_msg {
                    if let Some(msg) = {
                        trace!("processing message: {:?}", msg);
                        match msg {
                            SyncMessage::Doc(msg) => match msg {
                                DocMessage::Step1(sv) => {
                                    StateVector::decode_v1(&sv).ok().and_then(|sv| {
                                        trx.encode_state_as_update_v1(&sv)
                                            .map(|update| {
                                                SyncMessage::Doc(DocMessage::Step2(update))
                                            })
                                            .ok()
                                    })
                                }
                                DocMessage::Step2(update) => {
                                    if let Ok(update) = Update::decode_v1(&update) {
                                        trx.apply_update(update);
                                    }
                                    None
                                }
                                DocMessage::Update(update) => {
                                    if let Ok(update) = Update::decode_v1(&update) {
                                        trx.apply_update(update);
                                        trx.commit();
                                        if cfg!(debug_assertions) {
                                            trace!(
                                                "changed_parent_types: {:?}",
                                                trx.changed_parent_types()
                                            );
                                            trace!("before_state: {:?}", trx.before_state());
                                            trace!("after_state: {:?}", trx.after_state());
                                        }
                                        trx.encode_update_v1()
                                            .map(|update| {
                                                SyncMessage::Doc(DocMessage::Update(update))
                                            })
                                            .ok()
                                    } else {
                                        None
                                    }
                                }
                            },
                            _ => None,
                        }
                    } {
                        let mut buffer = Vec::new();
                        if let Err(e) = write_sync_message(&mut buffer, &msg) {
                            warn!("failed to encode message: {:?}", e);
                        } else {
                            result.push(buffer);
                        }
                    }
                }
            })) {
                warn!("failed to apply update: {:?}", e);
            }
            self.try_subscribe_all_blocks();
        }

        result
    }

    pub fn try_subscribe_all_blocks(&mut self) {
        if let Some(block_observer_config) = self.block_observer_config.clone() {
            // costing approximately 1ms per 500 blocks
            if let Err(e) = self.retry_with_trx(
                |mut t| {
                    t.get_blocks().blocks(&t.trx, |blocks| {
                        blocks.for_each(|mut block| {
                            let block_observer_config = block_observer_config.clone();
                            block.subscribe(block_observer_config);
                        })
                    });
                },
                10,
            ) {
                error!("subscribe synchronized block callback failed: {}", e);
            }
        }
    }
}
