use std::sync::Arc;

use super::{plugins::setup_plugin, *};
use serde::{ser::SerializeMap, Serialize, Serializer};
use y_sync::{
    awareness::{Awareness, Event, Subscription as AwarenessSubscription},
    sync::{DefaultProtocol, Error, Message, MessageReader, Protocol, SyncMessage},
};
use yrs::{
    types::{map::MapEvent, ToJson},
    updates::{
        decoder::{Decode, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
    Doc, Map, MapRef, Observable, ReadTxn, StateVector, Subscription, Transact, TransactionMut,
    Update, UpdateEvent, UpdateSubscription,
};

static PROTOCOL: DefaultProtocol = DefaultProtocol;

use super::PluginMap;
use plugins::PluginImpl;

type MapSubscription = Subscription<Arc<dyn Fn(&TransactionMut, &MapEvent)>>;

pub struct Workspace {
    id: String,
    awareness: Awareness,
    pub(crate) blocks: MapRef,
    pub(crate) updated: MapRef,
    pub(crate) metadata: MapRef,
    /// We store plugins so that their ownership is tied to [Workspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [Workspace] experiences, like block updates.
    ///
    /// Public just for the crate as we experiment with the plugins interface.
    /// See [plugins].
    pub(super) plugins: PluginMap,
}

unsafe impl Send for Workspace {}
unsafe impl Sync for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        let doc = Doc::new();
        Self::from_doc(doc, id)
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, id: S) -> Workspace {
        let blocks = doc.get_or_insert_map("blocks");
        let updated = doc.get_or_insert_map("updated");
        let metadata = doc.get_or_insert_map("space:meta");

        let workspace = Self {
            id: id.as_ref().to_string(),
            awareness: Awareness::new(doc),
            blocks,
            updated,
            metadata,
            plugins: Default::default(),
        };

        setup_plugin(workspace)
    }

    /// Allow the plugin to run any necessary updates it could have flagged via observers.
    /// See [plugins].
    pub(super) fn update_plugin<P: PluginImpl>(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.plugins.update_plugin::<P>(self)
    }

    /// See [plugins].
    pub(super) fn with_plugin<P: PluginImpl, T>(&self, cb: impl Fn(&P) -> T) -> Option<T> {
        self.plugins.with_plugin::<P, T>(cb)
    }

    #[cfg(feature = "workspace-search")]
    pub fn search<S: AsRef<str>>(
        &mut self,
        options: S,
    ) -> Result<SearchResults, Box<dyn std::error::Error>> {
        use plugins::IndexingPluginImpl;

        // refresh index if doc has update
        self.update_plugin::<IndexingPluginImpl>()?;

        let options = options.as_ref();

        self.with_plugin::<IndexingPluginImpl, Result<SearchResults, Box<dyn std::error::Error>>>(
            |search_plugin| search_plugin.search(options),
        )
        .expect("text search was set up by default")
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> T {
        let trx = WorkspaceTransaction {
            trx: self.doc().transact_mut(),
            ws: self,
        };

        f(trx)
    }

    pub fn try_with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> Option<T> {
        match self.doc().try_transact_mut() {
            Ok(trx) => {
                let trx = WorkspaceTransaction { trx, ws: self };
                Some(f(trx))
            }
            Err(e) => {
                info!("try_with_trx error: {}", e);
                None
            }
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn metadata(&self) -> WorkspaceMetadata {
        (&self.doc().transact(), self.metadata.clone()).into()
    }

    pub fn client_id(&self) -> u64 {
        self.doc().client_id()
    }

    // get a block if exists
    pub fn get<T, S>(&self, trx: &T, block_id: S) -> Option<Block>
    where
        T: ReadTxn,
        S: AsRef<str>,
    {
        Block::from(trx, self, block_id, self.client_id())
    }

    pub fn block_count(&self) -> u32 {
        self.blocks.len(&self.doc().transact())
    }

    #[inline]
    pub fn blocks<T, R>(&self, trx: &T, cb: impl Fn(Box<dyn Iterator<Item = Block> + '_>) -> R) -> R
    where
        T: ReadTxn,
    {
        let iterator =
            self.blocks
                .iter(trx)
                .zip(self.updated.iter(trx))
                .map(|((id, block), (_, updated))| {
                    Block::from_raw_parts(
                        trx,
                        id.to_owned(),
                        self.doc(),
                        block.to_ymap().unwrap(),
                        updated.to_yarray().unwrap(),
                        self.client_id(),
                    )
                });

        cb(Box::new(iterator))
    }

    pub fn observe_metadata(
        &mut self,
        f: impl Fn(&TransactionMut, &MapEvent) + 'static,
    ) -> MapSubscription {
        self.metadata.observe(f)
    }

    pub fn on_awareness_update(
        &mut self,
        f: impl Fn(&Awareness, &Event) + 'static,
    ) -> AwarenessSubscription<Event> {
        self.awareness.on_update(f)
    }

    /// Check if the block exists in this workspace's blocks.
    pub fn exists<T>(&self, trx: &T, block_id: &str) -> bool
    where
        T: ReadTxn,
    {
        self.blocks.contains_key(trx, block_id.as_ref())
    }

    /// Subscribe to update events.
    pub fn observe(
        &mut self,
        f: impl Fn(&TransactionMut, &UpdateEvent) + 'static,
    ) -> Option<UpdateSubscription> {
        self.awareness.doc_mut().observe_update_v1(f).ok()
    }

    pub fn doc(&self) -> &Doc {
        self.awareness.doc()
    }

    pub fn sync_migration(&self) -> Vec<u8> {
        self.doc()
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    pub fn sync_init_message(&self) -> Result<Vec<u8>, Error> {
        let mut encoder = EncoderV1::new();
        PROTOCOL.start(&self.awareness, &mut encoder)?;
        Ok(encoder.to_vec())
    }

    pub fn sync_handle_message(&mut self, msg: Message) -> Result<Option<Message>, Error> {
        debug!("processing message: {:?}", msg);
        match msg {
            Message::Sync(msg) => match msg {
                SyncMessage::SyncStep1(sv) => PROTOCOL.handle_sync_step1(&self.awareness, sv),
                SyncMessage::SyncStep2(update) => {
                    PROTOCOL.handle_sync_step2(&mut self.awareness, Update::decode_v1(&update)?)
                }
                SyncMessage::Update(update) => {
                    let mut txn = self.doc().transact_mut();
                    txn.apply_update(Update::decode_v1(&update)?);
                    txn.commit();
                    debug!("changed_parent_types: {:?}", txn.changed_parent_types());
                    debug!("before_state: {:?}", txn.before_state());
                    debug!("after_state: {:?}", txn.after_state());
                    let update = txn.encode_update_v1();
                    Ok(Some(Message::Sync(SyncMessage::Update(update))))
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

impl Serialize for Workspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let trx = self.doc().transact();
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("blocks", &self.blocks.to_json(&trx))?;
        map.serialize_entry("updated", &self.updated.to_json(&trx))?;
        map.end()
    }
}

impl Clone for Workspace {
    fn clone(&self) -> Self {
        let id = self.id.clone();
        let doc = self.doc().clone();
        Self::from_doc(doc, id)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use log::info;
    use yrs::{updates::decoder::Decode, Doc, StateVector, Update};

    #[test]
    fn doc_load_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let block = t.create("test", "text");

            block.set(&mut t.trx, "test", "test");
        });

        let doc = workspace.doc();

        let new_doc = {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());
            let doc = Doc::default();
            {
                let mut trx = doc.transact_mut();
                match Update::decode_v1(&update) {
                    Ok(update) => trx.apply_update(update),
                    Err(err) => info!("failed to decode update: {:?}", err),
                }
                trx.commit();
            }
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.get_or_insert_map("blocks").to_json(&doc.transact()),
            new_doc.get_or_insert_map("blocks").to_json(&doc.transact())
        );

        assert_json_diff::assert_json_eq!(
            doc.get_or_insert_map("updated").to_json(&doc.transact()),
            new_doc
                .get_or_insert_map("updated")
                .to_json(&doc.transact())
        );
    }

    #[test]
    fn workspace() {
        let workspace = Workspace::new("test");

        workspace.with_trx(|t| {
            assert_eq!(workspace.id(), "test");
            assert_eq!(workspace.blocks.len(&t.trx), 0);
            assert_eq!(workspace.updated.len(&t.trx), 0);
        });

        workspace.with_trx(|mut t| {
            let block = t.create("block", "text");

            assert_eq!(workspace.blocks.len(&t.trx), 1);
            assert_eq!(workspace.updated.len(&t.trx), 1);
            assert_eq!(block.id(), "block");
            assert_eq!(block.flavor(&t.trx), "text");

            assert_eq!(
                workspace.get(&t.trx, "block").map(|b| b.id()),
                Some("block".to_owned())
            );

            assert_eq!(workspace.exists(&t.trx, "block"), true);

            assert_eq!(t.remove("block"), true);

            assert_eq!(workspace.blocks.len(&t.trx), 0);
            assert_eq!(workspace.updated.len(&t.trx), 0);
            assert_eq!(workspace.get(&t.trx, "block"), None);
            assert_eq!(workspace.exists(&t.trx, "block"), false);
        });

        let doc = Doc::with_client_id(123);
        let workspace = Workspace::from_doc(doc, "test");
        assert_eq!(workspace.client_id(), 123);
    }
}
