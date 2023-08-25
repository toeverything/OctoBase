use jwst_codec::{
    write_sync_message, CrdtRead, CrdtWrite, DocMessage, RawDecoder, RawEncoder, StateVector, SyncMessage,
    SyncMessageScanner, Update,
};
use tracing::debug;

use super::*;

impl Workspace {
    pub fn sync_migration(&self) -> JwstResult<Vec<u8>> {
        Ok(self.doc.encode_state_as_update_v1(&StateVector::default())?)
    }

    pub async fn sync_init_message(&self) -> JwstResult<Vec<u8>> {
        let mut buffer = Vec::new();

        write_sync_message(
            &mut buffer,
            &SyncMessage::Doc(DocMessage::Step1({
                let mut encoder = RawEncoder::default();
                self.doc.get_state_vector().write(&mut encoder)?;
                encoder.into_inner()
            })),
        )?;
        write_sync_message(
            &mut buffer,
            &SyncMessage::Awareness(self.awareness.read().await.get_states().clone()),
        )?;

        Ok(buffer)
    }

    pub async fn sync_messages(&mut self, buffers: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        let mut awareness = vec![];
        let mut content = vec![];

        for buffer in buffers {
            let (awareness_msg, content_msg): (Vec<_>, Vec<_>) = SyncMessageScanner::new(&buffer)
                .flatten()
                .partition(|msg| matches!(msg, SyncMessage::Awareness(_) | SyncMessage::AwarenessQuery));

            debug!(
                "sync message: {}, awareness: {}, content: {}",
                buffer.len(),
                awareness_msg.len(),
                content_msg.len()
            );

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
                        if let Err(e) =
                            write_sync_message(&mut buffer, &SyncMessage::Awareness(awareness.get_states().clone()))
                        {
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
            let mut doc = self.doc();

            for msg in msg {
                if let Some(msg) = {
                    trace!("processing message: {:?}", msg);
                    match msg {
                        SyncMessage::Doc(msg) => match msg {
                            DocMessage::Step1(sv) => StateVector::read(&mut RawDecoder::new(sv)).ok().and_then(|sv| {
                                debug!("step1 get sv: {sv:?}");
                                doc.encode_state_as_update_v1(&sv)
                                    .map(|update| {
                                        debug!("step1 encode update: {}", update.len());
                                        SyncMessage::Doc(DocMessage::Step2(update))
                                    })
                                    .ok()
                            }),
                            DocMessage::Step2(update) => {
                                if let Ok(update) = Update::read(&mut RawDecoder::new(update)) {
                                    if let Err(e) = doc.apply_update(update) {
                                        warn!("failed to apply update: {:?}", e);
                                    }
                                }
                                None
                            }
                            DocMessage::Update(update) => doc
                                .apply_update_from_binary(update)
                                .and_then(|update| {
                                    if update.is_content_empty() {
                                        return Ok(None);
                                    }

                                    let mut encoder = RawEncoder::default();
                                    update.write(&mut encoder)?;
                                    Ok(Some(encoder.into_inner()))
                                })
                                .map_err(|e| warn!("failed to apply update: {:?}", e))
                                .ok()
                                .flatten()
                                .map(|u| SyncMessage::Doc(DocMessage::Update(u))),
                        },
                        _ => None,
                    }
                } {
                    let mut buffer = Vec::new();
                    if let Err(e) = write_sync_message(&mut buffer, &msg) {
                        warn!("failed to encode message: {:?}", e);
                    } else {
                        debug!("return update: {}", buffer.len());
                        result.push(buffer);
                    }
                }
            }
        }

        result
    }
}
