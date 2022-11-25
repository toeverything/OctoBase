use super::*;
use serde::{ser::SerializeMap, Serialize, Serializer};
use yrs::{
    types::ToJson, Doc, Map, MapRef, Transact, Transaction, TransactionMut, UpdateEvent,
    UpdateSubscription,
};

pub struct Workspace {
    id: String,
    doc: Doc,
    blocks: MapRef,
    updated: MapRef,
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

        Self {
            id: id.as_ref().to_string(),
            doc,
            blocks,
            updated,
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn blocks(&self) -> MapRef {
        self.blocks
    }

    pub fn updated(&self) -> MapRef {
        self.updated
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client_id
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> T {
        let trx = WorkspaceTransaction {
            trx: self.doc.transact(),
            trx_mut: self.doc.transact_mut(),
            ws: self,
        };

        f(trx)
    }

    pub fn get_trx(&self) -> WorkspaceTransaction {
        WorkspaceTransaction {
            trx: self.doc.transact(),
            trx_mut: self.doc.transact_mut(),
            ws: self,
        }
    }

    #[inline]
    pub fn block_iter(&self, trx: TransactionMut<'_>) -> impl Iterator<Item = Block> + '_ {
        self.blocks
            .iter()
            .zip(self.updated.iter())
            .map(|((id, block), (_, updated))| {
                Block::from_raw_parts(
                    id.to_owned(),
                    block.to_ymap().unwrap(),
                    updated.to_yarray().unwrap(),
                    self.doc.client_id,
                )
            })
    }

    pub fn observe(
        &mut self,
        f: impl Fn(&TransactionMut, &UpdateEvent) -> () + 'static,
    ) -> Option<UpdateSubscription> {
        self.doc.observe_update_v1(f).ok()
    }
}

impl Serialize for Workspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        let trx = self.doc.transact();
        map.serialize_entry("blocks", &self.blocks.to_json(&trx))?;
        map.serialize_entry("updated", &self.updated.to_json(&trx))?;
        map.end()
    }
}

pub struct WorkspaceTransaction<'a> {
    pub ws: &'a Workspace,
    pub trx: Transaction<'a>,
    pub trx_mut: TransactionMut<'a>,
}

unsafe impl Send for WorkspaceTransaction<'_> {}

impl WorkspaceTransaction<'_> {
    pub fn remove<S: AsRef<str>>(&mut self, block_id: S) -> bool {
        self.ws
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
            self.ws.doc.client_id,
        )
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S) -> Option<Block>
    where
        S: AsRef<str>,
    {
        Block::from(self.ws, self.trx, block_id, self.ws.doc.client_id)
    }

    pub fn exists(&self, block_id: &str) -> bool {
        self.ws.blocks.contains(&self.trx, block_id.as_ref())
    }

    pub fn block_count(&self) -> u32 {
        self.ws.blocks.len(&self.trx)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use yrs::Doc;

    #[test]
    fn workspace() {
        let workspace = Workspace::new("test");

        let mut trx = workspace.get_trx();

        assert_eq!(workspace.id(), "test");
        assert_eq!(workspace.blocks().len(&mut trx.trx), 0);
        assert_eq!(workspace.updated().len(&mut trx.trx), 0);

        let block = trx.create("block", "text");

        assert_eq!(workspace.blocks().len(&mut trx.trx), 1);
        assert_eq!(workspace.updated().len(&mut trx.trx), 1);
        assert_eq!(block.id(), "block");
        assert_eq!(block.flavor(&mut trx.trx), "text");

        assert_eq!(trx.get("block").map(|b| b.id()), Some("block".to_owned()));

        assert_eq!(trx.exists("block"), true);
        assert_eq!(trx.remove("block"), true);
        assert_eq!(workspace.blocks().len(&mut trx.trx), 0);
        assert_eq!(workspace.updated().len(&mut trx.trx), 0);
        assert_eq!(trx.get("block"), None);

        assert_eq!(trx.exists("block"), false);

        let doc = Doc::with_client_id(123);
        let workspace = Workspace::from_doc(doc, "test");
        assert_eq!(workspace.client_id(), 123);
    }
}
