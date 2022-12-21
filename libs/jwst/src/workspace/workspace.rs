use crate::utils::JS_INT_RANGE;

use super::{plugins::setup_plugin, *};
use lib0::any::Any;
use serde::{ser::SerializeMap, Serialize, Serializer};
use y_sync::{
    awareness::Awareness,
    sync::{Error, Message},
};
use yrs::{Doc, Map, Subscription, Transaction, UpdateEvent};

use super::PluginMap;
use plugins::PluginImpl;

pub struct Workspace {
    content: Content,
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
        let mut trx = doc.transact();

        // TODO: Initial index in Tantivy including:
        //  * Tree visitor collecting all child blocks which are correct flavor for extracted text
        //  * Extract prop:text / prop:title for index to block ID in Tantivy
        let blocks = trx.get_map("blocks");
        let updated = trx.get_map("updated");
        let metadata = trx.get_map("space:meta");

        let workspace = Self {
            content: Content {
                id: id.as_ref().to_string(),
                awareness: Awareness::new(doc),
                blocks,
                updated,
                metadata,
            },
            plugins: Default::default(),
        };

        setup_plugin(workspace)
    }

    /// Allow the plugin to run any necessary updates it could have flagged via observers.
    /// See [plugins].
    pub(super) fn update_plugin<P: PluginImpl>(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.plugins.update_plugin::<P>(&self.content)
    }

    /// See [plugins].
    pub(super) fn get_plugin<P: PluginImpl>(&self) -> Option<&P> {
        self.plugins.get_plugin::<P>()
    }

    #[cfg(feature = "workspace-search")]
    pub fn search<S: AsRef<str>>(
        &mut self,
        options: S,
    ) -> Result<SearchResults, Box<dyn std::error::Error>> {
        use plugins::IndexingPluginImpl;

        // refresh index if doc has update
        self.update_plugin::<IndexingPluginImpl>()?;

        let search_plugin = self
            .get_plugin::<IndexingPluginImpl>()
            .expect("text search was set up by default");
        search_plugin.search(options)
    }

    pub fn content(&self) -> &Content {
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

    pub fn metadata(&self) -> &Map {
        &self.content.metadata
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

    pub fn sync_decode_message(&mut self, binary: &[u8]) -> Vec<Vec<u8>> {
        self.content.sync_decode_message(binary)
    }

    pub fn sync_handle_message(&mut self, msg: Message) -> Result<Option<Message>, Error> {
        self.content.sync_handle_message(msg)
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

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) {
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.ws.metadata().insert(&mut self.trx, key, bool);
            }
            Any::String(text) => {
                self.ws
                    .metadata()
                    .insert(&mut self.trx, key, text.to_string());
            }
            Any::Number(number) => {
                self.ws.metadata().insert(&mut self.trx, key, number);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.ws.metadata().insert(&mut self.trx, key, number as f64);
                } else {
                    self.ws.metadata().insert(&mut self.trx, key, number);
                }
            }
            Any::Null | Any::Undefined => {
                self.ws.metadata().remove(&mut self.trx, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }
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
