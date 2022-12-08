use std::any::Any;

use super::*;
use serde::{ser::SerializeMap, Serialize, Serializer};
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

mod plugins;
#[cfg(feature = "workspace-search")]
mod search_indexing;

use plugins::WorkspacePluginMap;
pub(crate) use plugins::{WorkspacePlugin, WorkspacePluginConfig};

#[cfg(feature = "workspace-search")]
pub use search_indexing::{SearchBlockItem, SearchBlockList, SearchQueryOptions};

static PROTOCOL: DefaultProtocol = DefaultProtocol;

// Is the workspace here supposed to contain a source of truth for the
// block data?
pub struct WorkspaceContent {
    id: String,
    // indexers: Vec<Box<dyn search_indexing::BlockIndexer>>,
    awareness: Awareness,
    // Are these the blocks as committed / synced?
    blocks: Map,
    // What is this?
    updated: Map,
}

pub struct Workspace {
    content: WorkspaceContent,
    /// We store plugins so that their ownership is tied to [Workspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [Workspace] experiences, like block updates.
    ///
    /// Public just for the crate as we experiment with the plugins interface.
    /// See [plugins].
    pub(crate) plugins: WorkspacePluginMap,
    // /// Global version of the workspace
    // ///
    // /// This is used by extensions such as Text Search to determine
    // /// what blocks have changed since their last indexing.
    // ///
    // /// This makes extensions behave more like a pull-based system.
    // local_version: u64,
}

unsafe impl Send for Workspace {}
unsafe impl Sync for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        let doc = Doc::new();
        Self::from_doc(doc, id)
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, id: S) -> Workspace {
        let mut workspace = Self::from_doc_without_plugins(doc, id);

        // default plugins
        #[cfg(feature = "workspace-search")]
        {
            // Set up search
            workspace.setup_plugin(search_indexing::WorkspaceTextSearchPluginConfig {
                // should the default storage go somewhere else?
                storage_kind: search_indexing::WorkspaceTextSearchPluginConfigStorageKind::RAM,
            });
        }

        workspace
    }

    /// Crate public interface to create the workspace with no plugins for testing.
    pub(crate) fn from_doc_without_plugins<S: AsRef<str>>(doc: Doc, id: S) -> Workspace {
        let mut trx = doc.transact();

        // TODO: Initial index in Tantivy including:
        //  * Tree visitor collecting all child blocks which are correct flavor for extracted text
        //  * Extract prop:text / prop:title for index to block ID in Tantivy
        let blocks = trx.get_map("blocks");
        let updated = trx.get_map("updated");

        Self {
            content: WorkspaceContent {
                id: id.as_ref().to_string(),
                awareness: Awareness::new(doc),
                blocks,
                updated,
            },
            plugins: Default::default(),
        }
    }

    /// Setup a [WorkspacePlugin] and insert it into the [Workspace].
    /// See [plugins].
    pub(crate) fn setup_plugin(
        &mut self,
        config: impl WorkspacePluginConfig,
    ) -> Result<&mut Self, Box<dyn std::error::Error>> {
        let plugin = config.setup(self)?;
        self.plugins.insert_plugin(plugin)?;
        Ok(self)
    }

    /// Allow the plugin to run any necessary updates it could have flagged via observers.
    /// See [plugins].
    pub(crate) fn update_plugin<P: WorkspacePlugin>(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.plugins.update_plugin::<P>(&self.content)
    }

    /// See [plugins].
    pub(crate) fn get_plugin<P: WorkspacePlugin>(&self) -> Option<&P> {
        self.plugins.get_plugin::<P>()
    }

    #[cfg(feature = "workspace-search")]
    pub fn search(
        &self,
        options: &SearchQueryOptions,
    ) -> Result<SearchBlockList, Box<dyn std::error::Error>> {
        let search_plugin = self
            .get_plugin::<search_indexing::WorkspaceTextSearchPlugin>()
            .expect("text search was set up by default");
        search_plugin.search(options)
    }

    #[cfg(feature = "workspace-search")]
    pub fn update_search_index(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_plugin::<search_indexing::WorkspaceTextSearchPlugin>()
    }

    pub fn content(&self) -> &WorkspaceContent {
        &self.content
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> T {
        let trx = WorkspaceTransaction {
            trx: self.content.awareness.doc().transact(),
            ws: self,
        };

        f(trx)
    }

    pub fn get_trx(&self) -> WorkspaceTransaction {
        WorkspaceTransaction {
            trx: self.content.awareness.doc().transact(),
            ws: self,
        }
    }
    pub fn id(&self) -> String {
        self.content.id()
    }

    pub fn blocks(&self) -> &Map {
        self.content.blocks()
    }

    pub fn updated(&self) -> &Map {
        self.content.updated()
    }

    pub fn doc(&self) -> &Doc {
        self.content.doc()
    }

    pub fn client_id(&self) -> u64 {
        self.content.client_id()
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S) -> Option<Block>
    where
        S: AsRef<str>,
    {
        self.content.get(block_id)
    }

    pub fn block_count(&self) -> u32 {
        self.content.block_count()
    }

    #[inline]
    pub fn block_iter(&self) -> impl Iterator<Item = Block> + '_ {
        self.content.block_iter()
    }

    /// Check if the block exists in this workspace's blocks.
    pub fn exists(&self, block_id: &str) -> bool {
        self.content.exists(block_id)
    }

    /// Subscribe to update events.
    pub fn observe(
        &mut self,
        f: impl Fn(&Transaction, &UpdateEvent) -> () + 'static,
    ) -> Subscription<UpdateEvent> {
        self.content.observe(f)
    }

    pub fn sync_migration(&self) -> Vec<u8> {
        self.content.sync_migration()
    }

    pub fn sync_init_message(&self) -> Result<Vec<u8>, Error> {
        self.content.sync_init_message()
    }

    fn sync_handle_message(&mut self, msg: Message) -> Result<Option<Message>, Error> {
        self.content.sync_handle_message(msg)
    }

    pub fn sync_decode_message(&mut self, binary: &[u8]) -> Vec<Vec<u8>> {
        self.content.sync_decode_message(binary)
    }
}

impl WorkspaceContent {
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

    fn sync_handle_message(&mut self, msg: Message) -> Result<Option<Message>, Error> {
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

impl Serialize for Workspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("blocks", &self.content.blocks.to_json())?;
        map.serialize_entry("updated", &self.content.updated.to_json())?;
        map.end()
    }
}

pub struct WorkspaceTransaction<'a> {
    pub ws: &'a Workspace,
    pub trx: Transaction,
}

unsafe impl Send for WorkspaceTransaction<'_> {}

impl WorkspaceTransaction<'_> {
    pub fn remove<S: AsRef<str>>(&mut self, block_id: S) -> bool {
        self.ws
            .content
            .blocks
            .remove(&mut self.trx, block_id.as_ref())
            .is_some()
            && self
                .ws
                .updated()
                .remove(&mut self.trx, block_id.as_ref())
                .is_some()
    }

    // create a block with specified flavor
    // if block exists, return the exists block
    pub fn create<B, F>(&mut self, block_id: B, flavor: F) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        Block::new(
            self.ws,
            &mut self.trx,
            block_id,
            flavor,
            self.ws.client_id(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use yrs::Doc;

    #[test]
    fn doc_load_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let block = t.create("test", "text");

            block.set(&mut t.trx, "test", "test");
        });

        let doc = workspace.doc();

        let new_doc = {
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            let doc = Doc::default();
            let mut trx = doc.transact();
            match Update::decode_v1(&update) {
                Ok(update) => trx.apply_update(update),
                Err(err) => info!("failed to decode update: {:?}", err),
            }
            trx.commit();
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("blocks").to_json(),
            new_doc.transact().get_map("blocks").to_json()
        );

        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("updated").to_json(),
            new_doc.transact().get_map("updated").to_json()
        );
    }

    #[test]
    fn workspace() {
        let workspace = Workspace::new("test");

        assert_eq!(workspace.id(), "test");
        assert_eq!(workspace.blocks().len(), 0);
        assert_eq!(workspace.updated().len(), 0);

        let block = workspace.get_trx().create("block", "text");
        assert_eq!(workspace.blocks().len(), 1);
        assert_eq!(workspace.updated().len(), 1);
        assert_eq!(block.id(), "block");
        assert_eq!(block.flavor(), "text");

        assert_eq!(
            workspace.get("block").map(|b| b.id()),
            Some("block".to_owned())
        );

        assert_eq!(workspace.exists("block"), true);
        assert_eq!(workspace.get_trx().remove("block"), true);
        assert_eq!(workspace.blocks().len(), 0);
        assert_eq!(workspace.updated().len(), 0);
        assert_eq!(workspace.get("block"), None);

        assert_eq!(workspace.exists("block"), false);

        let doc = Doc::with_client_id(123);
        let workspace = Workspace::from_doc(doc, "test");
        assert_eq!(workspace.client_id(), 123);
    }
}

#[cfg(all(test, feature = "workspace-search"))]
mod test_search {
    //! Consider moving this code out of the crate so we can properly
    //! demonstrate and validate that this integration testing
    //! is exactly how another crate would be able to use workspace search.

    use super::*;
    use yrs::Doc;

    // out of order for now, in the future, this can be made in order by sorting before
    // we reduce to just the block ids. Then maybe we could first sort on score, then sort on
    // block id.
    macro_rules! expect_result_ids {
        ($search_results:ident, $id_str_array:expr) => {
            let mut sorted_ids = $search_results
                .items
                .iter()
                .map(|i| &i.block_id)
                .collect::<Vec<_>>();
            sorted_ids.sort();
            assert_eq!(
                sorted_ids, $id_str_array,
                "Expected found ids (left) match expected ids (right) for search results"
            );
        };
    }
    macro_rules! expect_search_gives_ids {
        ($workspace:ident, $query_text:expr, $id_str_array:expr) => {
            let search_result = $workspace
                .search(&SearchQueryOptions {
                    query: $query_text.to_string(),
                })
                .expect("no error searching");

            let line = line!();
            println!("Search results (workspace.rs:{line}): {search_result:#?}"); // will show if there is an issue running the test

            expect_result_ids!(search_result, $id_str_array);
        };
    }

    #[test]
    fn basic_search_test() {
        // default workspace should be set up with search plugin under these features.
        let mut wk = Workspace::from_doc(Default::default(), "wk-load");

        wk.with_trx(|mut t| {
            let block = t.create("b1", "text");
            block.set(&mut t.trx, "test", "test");

            let block = t.create("a", "affine:text");
            let b = t.create("b", "affine:text");
            let c = t.create("c", "affine:text");
            let d = t.create("d", "affine:text");
            let e = t.create("e", "affine:text");
            let f = t.create("f", "affine:text");
            let trx = &mut t.trx;

            b.set(trx, "title", "Title B content");
            b.set(trx, "text", "Text B content bbb xxx");

            c.set(trx, "title", "Title C content");
            c.set(trx, "text", "Text C content ccc xxx yyy");

            d.set(trx, "title", "Title D content");
            d.set(trx, "text", "Text D content ddd yyy");
            // assert_eq!(b.get(trx, "title"), "Title content");
            // b.set(trx, "text", "Text content");

            // pushing blocks in
            {
                block.push_children(trx, &b);
                block.insert_children_at(trx, &c, 0);
                block.insert_children_before(trx, &d, "b");
                block.insert_children_after(trx, &e, "b");
                block.insert_children_after(trx, &f, "c");

                assert_eq!(
                    block.children(),
                    vec![
                        "c".to_owned(),
                        "f".to_owned(),
                        "d".to_owned(),
                        "b".to_owned(),
                        "e".to_owned()
                    ]
                );
            }

            // Question: Is this supposed to indicate that since this block is detached, then we should not be indexing it?
            // For example, should we walk up the parent tree to check if each block is actually attached?
            block.remove_children(trx, &d);
        });

        println!("Blocks: {:#?}", wk.blocks()); // shown if there is an issue running the test.

        // update search index before making new queries
        wk.update_search_index().expect("update text search plugin");

        expect_search_gives_ids!(wk, "content", &["b", "c", "d"]);
        expect_search_gives_ids!(wk, "bbb", &["b"]);
        expect_search_gives_ids!(wk, "ccc", &["c"]);
        expect_search_gives_ids!(wk, "xxx", &["b", "c"]);
        expect_search_gives_ids!(wk, "yyy", &["c", "d"]);
    }
}
