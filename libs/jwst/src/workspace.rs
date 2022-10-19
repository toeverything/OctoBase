use super::*;
use lib0::any::Any;
use serde::{ser::SerializeMap, Serialize, Serializer};
use yrs::{Doc, Map, PrelimMap, Subscription, Transaction, UpdateEvent};

pub struct Workspace {
    id: String,
    blocks: Map,
    updated: Map,
    doc: Doc,
}

unsafe impl Send for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        let doc = Doc::new();
        Self::from_doc(doc, id)
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, id: S) -> Workspace {
        let mut trx = doc.transact();
        let blocks = trx.get_map("blocks");
        // blocks.content
        let content = blocks
            .get("content")
            .or_else(|| {
                blocks.insert(&mut trx, "content", PrelimMap::<Any>::new());
                blocks.get("content")
            })
            .and_then(|b| b.to_ymap())
            .unwrap();

        // blocks.updated
        let updated = blocks
            .get("updated")
            .or_else(|| {
                blocks.insert(&mut trx, "updated", PrelimMap::<Any>::new());
                blocks.get("updated")
            })
            .and_then(|b| b.to_ymap())
            .unwrap();
        Self {
            id: id.as_ref().to_string(),
            blocks: content,
            updated,
            doc,
        }
    }

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
        &self.doc
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client_id
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(WorkspaceTranscation) -> T) -> T {
        let trx = WorkspaceTranscation {
            trx: self.doc.transact(),
            ws: self,
        };

        f(trx)
    }

    pub fn get_trx(&self) -> WorkspaceTranscation {
        WorkspaceTranscation {
            trx: self.doc.transact(),
            ws: self,
        }
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S) -> Option<Block>
    where
        S: AsRef<str>,
    {
        Block::from(self, block_id, self.doc.client_id)
    }

    pub fn exists(&self, block_id: &str) -> bool {
        self.blocks.contains(block_id.as_ref())
    }

    pub fn observe(
        &mut self,
        f: impl Fn(&Transaction, &UpdateEvent) -> () + 'static,
    ) -> Subscription<UpdateEvent> {
        self.doc.observe_update_v1(f)
    }
}

impl Serialize for Workspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("content", &self.blocks.to_json())?;
        map.serialize_entry("updated", &self.updated.to_json())?;
        map.end()
    }
}

pub struct WorkspaceTranscation<'a> {
    pub ws: &'a Workspace,
    pub trx: Transaction,
}

impl WorkspaceTranscation<'_> {
    pub fn remove(&mut self, block_id: &str) -> bool {
        self.ws
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
    pub fn create<B>(&mut self, block_id: B, flavor: &str) -> Block
    where
        B: AsRef<str>,
    {
        Block::new(
            self.ws,
            &mut self.trx,
            block_id,
            flavor,
            self.ws.doc.client_id,
        )
    }
}

#[cfg(test)]
mod test {
    use super::Workspace;

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
    }
}
