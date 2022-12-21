use super::*;
use y_sync::{
    awareness::Awareness,
    sync::{DefaultProtocol, Error, Message, MessageReader, Protocol, SyncMessage},
};
use yrs::{
    updates::{
        decoder::{Decode, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
    Doc, Map, StateVector, Subscription, Transaction, Update, UpdateEvent,
};

static PROTOCOL: DefaultProtocol = DefaultProtocol;

// Is the workspace here supposed to contain a source of truth for the
// block data?
pub struct Content {
    pub(super) id: String,
    pub(super) awareness: Awareness,
    pub(super) blocks: Map,
    pub(super) updated: Map,
    pub(super) metadata: Map,
}

impl Content {
    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn blocks(&self) -> &Map {
        &self.blocks
    }

    pub fn updated(&self) -> &Map {
        &self.updated
    }

    pub fn doc(&self) -> &Doc {
        self.awareness.doc()
    }

    pub fn client_id(&self) -> u64 {
        self.awareness.doc().client_id
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S) -> Option<Block>
    where
        S: AsRef<str>,
    {
        Block::from(self, block_id, self.client_id())
    }

    pub fn block_count(&self) -> u32 {
        self.blocks.len()
    }

    #[inline]
    pub fn block_iter<'a>(&'a self) -> impl Iterator<Item = Block> + 'a {
        self.blocks
            .iter()
            .zip(self.updated.iter())
            .map(|((id, block), (_, updated))| {
                Block::from_raw_parts(
                    id.to_owned(),
                    block.to_ymap().unwrap(),
                    updated.to_yarray().unwrap(),
                    self.client_id(),
                )
            })
    }

    /// Check if the block exists in this workspace's blocks.
    pub fn exists(&self, block_id: &str) -> bool {
        self.blocks.contains(block_id.as_ref())
    }

    /// Subscribe to update events.
    pub fn observe(
        &mut self,
        f: impl Fn(&Transaction, &UpdateEvent) -> () + 'static,
    ) -> Subscription<UpdateEvent> {
        self.awareness.doc_mut().observe_update_v1(f)
    }

    pub fn sync_migration(&self) -> Vec<u8> {
        self.doc()
            .encode_state_as_update_v1(&StateVector::default())
    }

    pub fn sync_init_message(&self) -> Result<Vec<u8>, Error> {
        let mut encoder = EncoderV1::new();
        PROTOCOL.start(&self.awareness, &mut encoder)?;
        Ok(encoder.to_vec())
    }

    pub fn sync_handle_message(&mut self, msg: Message) -> Result<Option<Message>, Error> {
        match msg {
            Message::Sync(msg) => match msg {
                SyncMessage::SyncStep1(sv) => PROTOCOL.handle_sync_step1(&self.awareness, sv),
                SyncMessage::SyncStep2(update) => {
                    PROTOCOL.handle_sync_step2(&mut self.awareness, Update::decode_v1(&update)?)
                }
                SyncMessage::Update(update) => {
                    PROTOCOL.handle_update(&mut self.awareness, Update::decode_v1(&update)?)
                }
            },
            Message::Auth(reason) => PROTOCOL.handle_auth(&self.awareness, reason),
            Message::AwarenessQuery => PROTOCOL.handle_awareness_query(&self.awareness),
            Message::Awareness(update) => {
                PROTOCOL.handle_awareness_update(&mut self.awareness, update)
            }
            Message::Custom(tag, data) => PROTOCOL.missing_handle(&mut self.awareness, tag, data),
        }
    }

    pub fn sync_decode_message(&mut self, binary: &[u8]) -> Vec<Vec<u8>> {
        let mut decoder = DecoderV1::from(binary);

        MessageReader::new(&mut decoder)
            .filter_map(|msg| msg.ok().and_then(|msg| self.sync_handle_message(msg).ok()?))
            .map(|reply| reply.encode_v1())
            .collect()
    }
}
