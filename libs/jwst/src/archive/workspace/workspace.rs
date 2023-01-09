#![allow(deprecated)]
use crate::archive::block::Block;
use crate::utils::JS_INT_RANGE;

use super::*;
use lib0::any::Any;
use serde::{ser::SerializeMap, Serialize, Serializer};
use y_sync::{
    awareness::Awareness,
    sync::{Error, Message},
};

use yrs::{
    types::map::MapEvent, types::ToJson, Doc, Map, MapRef, Subscription, Transact, TransactionMut,
    UpdateEvent, UpdateSubscription,
};

use super::Content;

#[deprecated = "Use OctoWorkspace and OctoWorkspaceRef"]
pub struct Workspace {
    content: Content,
    // /// We store plugins so that their ownership is tied to [Workspace].
    // /// This enables us to properly manage lifetimes of observers which will subscribe
    // /// into events that the [Workspace] experiences, like block updates.
    // ///
    // /// Public just for the crate as we experiment with the plugins interface.
    // /// See [plugins].
    // pub(super) plugins: PluginMap,
}

unsafe impl Send for Workspace {}
unsafe impl Sync for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        let doc = Doc::new();
        Self::from_doc(doc, id)
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, id: S) -> Workspace {
        // let mut trx = doc.transact();

        // TODO: Initial index in Tantivy including:
        //  * Tree visitor collecting all child blocks which are correct flavor for extracted text
        //  * Extract prop:text / prop:title for index to block ID in Tantivy
        let blocks = doc.get_or_insert_map("blocks");
        let updated = doc.get_or_insert_map("updated");
        let metadata = doc.get_or_insert_map("space:meta");

        let workspace = Self {
            content: Content {
                id: id.as_ref().to_string(),
                awareness: Awareness::new(doc.clone()),
                doc,
                blocks,
                updated,
                metadata,
            },
        };

        // setup_plugin(workspace)
        workspace
    }

    pub fn blocks(&self) -> &MapRef {
        self.content.blocks()
    }

    pub fn updated(&self) -> &MapRef {
        self.content.updated()
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> T {
        let trx = WorkspaceTransaction {
            trx_mut: self.doc().transact_mut(),
            ws: self,
        };

        f(trx)
    }

    pub fn get_trx(&self) -> WorkspaceTransaction {
        WorkspaceTransaction {
            trx_mut: self.doc().transact_mut(),
            ws: self,
        }
    }
    pub fn id(&self) -> String {
        self.content.id()
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

    // #[inline]
    // pub fn block_iter(&self) -> impl Iterator<Item = Block> + '_ {
    //     self.content.block_iter()
    // }

    pub fn metadata(&self) -> &MapRef {
        &self.content.metadata
    }

    pub fn observe_metadata(
        &mut self,
        f: impl Fn(&yrs::Transaction, &MapEvent) -> () + 'static,
    ) -> y_sync::awareness::Subscription<MapEvent> {
        // self.content.metadata.observe(f)
        todo!("self.content.metadata.observe(f)")
    }

    /// Check if the block exists in this workspace's blocks.
    pub fn exists(&self, block_id: &str) -> bool {
        self.content.exists(block_id)
    }

    // pub fn observe(
    //     &mut self,
    //     f: impl Fn(&TransactionMut, &UpdateEvent) -> () + 'static,
    // ) -> Option<UpdateSubscription> {
    //     self.content.observe(f)
    // }

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
        let read_txn = self.doc().transact();
        map.serialize_entry("blocks", &self.content.blocks.to_json(&read_txn))?;
        map.serialize_entry("updated", &self.content.updated.to_json(&read_txn))?;
        map.end()
    }
}

#[deprecated = "Use OctoRead or OctoWrite"]
pub struct WorkspaceTransaction<'a> {
    pub ws: &'a Workspace,
    pub trx_mut: TransactionMut<'a>,
}

unsafe impl Send for WorkspaceTransaction<'_> {}

impl WorkspaceTransaction<'_> {
    pub fn remove<S: AsRef<str>>(&mut self, block_id: S) -> bool {
        self.ws
            .content
            .blocks
            .remove(&mut self.trx_mut, block_id.as_ref())
            .is_some()
            && self
                .ws
                .updated()
                .remove(&mut self.trx_mut, block_id.as_ref())
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
            &self.trx_mut,
            block_id,
            flavor,
            self.ws.client_id(),
        )
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S) -> Option<Block>
    where
        S: AsRef<str>,
    {
        Block::from(
            self.ws.content(),
            &self.trx_mut,
            block_id,
            self.ws.client_id(),
        )
    }

    pub fn exists(&self, block_id: &str) -> bool {
        self.ws
            .content()
            .blocks()
            .contains(&self.trx_mut, block_id.as_ref())
    }

    pub fn block_count(&self) -> u32 {
        self.ws.content().blocks().len(&self.trx_mut)
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) {
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.ws.metadata().insert(&mut self.trx_mut, key, bool);
            }
            Any::String(text) => {
                self.ws
                    .metadata()
                    .insert(&mut self.trx_mut, key, text.to_string());
            }
            Any::Number(number) => {
                self.ws.metadata().insert(&mut self.trx_mut, key, number);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.ws
                        .metadata()
                        .insert(&mut self.trx_mut, key, number as f64);
                } else {
                    self.ws.metadata().insert(&mut self.trx_mut, key, number);
                }
            }
            Any::Null | Any::Undefined => {
                self.ws.metadata().remove(&mut self.trx_mut, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }
    }

    pub fn commit(&mut self) {
        self.trx_mut.commit();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use log::info;
    use yrs::{updates::decoder::Decode, Doc, ReadTxn, StateVector, Update};

    #[test]
    fn doc_load_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let block = t.create("test", "text");

            block.set(&mut t.trx_mut, "test", "test");
        });

        let doc = workspace.doc();

        let new_doc = {
            let doc = Doc::default();
            {
                let mut trx = doc.transact_mut();
                let update = trx.encode_state_as_update_v1(&StateVector::default());
                match Update::decode_v1(&update) {
                    Ok(update) => trx.apply_update(update),
                    Err(err) => info!("failed to decode update: {:?}", err),
                }
                trx.commit();
            }
            doc
        };

        let trx = doc.transact();
        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("blocks").unwrap().to_json(&trx),
            new_doc.transact().get_map("blocks").unwrap().to_json(&trx)
        );

        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("updated").unwrap().to_json(&trx),
            new_doc.transact().get_map("updated").unwrap().to_json(&trx)
        );
    }

    #[test]
    fn workspace() {
        let workspace = Workspace::new("test");

        let mut trx = workspace.get_trx();

        assert_eq!(workspace.id(), "test");
        assert_eq!(workspace.blocks().len(&trx.trx_mut), 0);
        assert_eq!(workspace.updated().len(&trx.trx_mut), 0);

        let block = trx.create("block", "text");

        assert_eq!(workspace.blocks().len(&trx.trx_mut), 1);
        assert_eq!(workspace.updated().len(&trx.trx_mut), 1);
        assert_eq!(block.id(), "block");
        assert_eq!(block.flavor(&trx.trx_mut), "text");

        assert_eq!(trx.get("block").map(|b| b.id()), Some("block".to_owned()));

        assert_eq!(trx.exists("block"), true);
        assert_eq!(trx.remove("block"), true);
        assert_eq!(workspace.blocks().len(&trx.trx_mut), 0);
        assert_eq!(workspace.updated().len(&trx.trx_mut), 0);
        assert_eq!(trx.get("block"), None);

        assert_eq!(trx.exists("block"), false);

        let doc = Doc::with_client_id(123);
        let workspace = Workspace::from_doc(doc, "test");
        assert_eq!(workspace.client_id(), 123);
    }
}
