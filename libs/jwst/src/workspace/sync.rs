use super::*;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    thread::sleep,
    time::Duration,
};
use y_sync::sync::{Message, MessageReader, SyncMessage};
use yrs::{
    updates::{
        decoder::{Decode, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
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
        let mut encoder = EncoderV1::new();
        let (sv, update) = {
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
            (sv, update)
        };

        Message::Sync(SyncMessage::SyncStep1(sv)).encode(&mut encoder)?;
        Message::Awareness(update).encode(&mut encoder)?;

        Ok(encoder.to_vec())
    }

    pub async fn sync_decode_message(&mut self, binary: &[u8]) -> Vec<Vec<u8>> {
        let mut decoder = DecoderV1::from(binary);
        let mut result = vec![];

        let (awareness_msg, content_msg): (Vec<_>, Vec<_>) = MessageReader::new(&mut decoder)
            .flatten()
            .partition(|msg| matches!(msg, Message::Awareness(_) | Message::AwarenessQuery));

        if !awareness_msg.is_empty() {
            let mut awareness = self.awareness.write().await;
            if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
                for msg in awareness_msg {
                    match msg {
                        Message::AwarenessQuery => {
                            if let Ok(update) = awareness.update() {
                                match Message::Awareness(update).encode_v1() {
                                    Ok(msg) => result.push(msg),
                                    Err(e) => warn!("failed to encode awareness update: {:?}", e),
                                }
                            }
                        }
                        Message::Awareness(update) => {
                            if let Err(e) = awareness.apply_update(update) {
                                warn!("failed to apply awareness: {:?}", e);
                            }
                        }
                        _ => {}
                    }
                }
            })) {
                warn!("failed to apply awareness update: {:?}", e);
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
                            Message::Sync(msg) => match msg {
                                SyncMessage::SyncStep1(sv) => trx
                                    .encode_state_as_update_v1(&sv)
                                    .map(|update| Message::Sync(SyncMessage::SyncStep2(update)))
                                    .ok(),
                                SyncMessage::SyncStep2(update) => {
                                    if let Ok(update) = Update::decode_v1(&update) {
                                        trx.apply_update(update);
                                    }
                                    None
                                }
                                SyncMessage::Update(update) => {
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
                                                Message::Sync(SyncMessage::Update(update))
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
                        match msg.encode_v1() {
                            Ok(msg) => result.push(msg),
                            Err(e) => warn!("failed to encode message: {:?}", e),
                        }
                    }
                }
            })) {
                warn!("failed to apply update: {:?}", e);
            }
        }

        result
    }
}
