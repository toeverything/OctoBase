use super::*;
use crate::RETRY_NUM;
use jwst_codec::{write_sync_message, DocMessage, SyncMessage, SyncMessageScanner};
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
    pub fn sync_migration(&self) -> JwstResult<Vec<u8>> {
        let mut retry = RETRY_NUM;
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
            let mut retry = RETRY_NUM;
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
            let sv: StateVector = trx.state_vector();
            let awareness = self.awareness.read().await;
            (sv, SyncMessage::Awareness(awareness.get_states().clone()))
        };

        let mut buffer = Vec::new();
        write_sync_message(
            &mut buffer,
            &SyncMessage::Doc(DocMessage::Step1(sv.encode_v1()?)),
        )?;
        write_sync_message(&mut buffer, &awareness_update)?;

        Ok(buffer)
    }

    pub async fn sync_messages(&mut self, buffers: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        let mut awareness = vec![];
        let mut content = vec![];

        for buffer in buffers {
            let (awareness_msg, content_msg): (Vec<_>, Vec<_>) =
                SyncMessageScanner::new(&buffer).flatten().partition(|msg| {
                    matches!(msg, SyncMessage::Awareness(_) | SyncMessage::AwarenessQuery)
                });
            awareness.extend(awareness_msg);
            content.extend(content_msg);
        }

        let mut result = vec![];

        result.extend(self.sync_awareness(awareness).await);
        result.extend(self.sync_content(content));

        result
    }

    async fn sync_awareness(&mut self, msgs: Vec<SyncMessage>) -> Vec<Vec<u8>> {
        let mut result = vec![];
        if !msgs.is_empty() {
            let mut awareness = self.awareness.write().await;
            for msg in msgs {
                match msg {
                    SyncMessage::AwarenessQuery => {
                        let mut buffer = Vec::new();
                        if let Err(e) = write_sync_message(
                            &mut buffer,
                            &SyncMessage::Awareness(awareness.get_states().clone()),
                        ) {
                            warn!("failed to encode awareness update: {:?}", e);
                        } else {
                            result.push(buffer);
                        }
                    }
                    SyncMessage::Awareness(update) => awareness.apply_update(update),
                    _ => {}
                }
            }
        }
        result
    }

    fn sync_content(&mut self, msg: Vec<SyncMessage>) -> Vec<Vec<u8>> {
        let mut result = vec![];
        if !msg.is_empty() {
            let doc = self.doc();
            if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
                let mut retry = RETRY_NUM;
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
                for msg in msg {
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
        }

        result
    }
}
