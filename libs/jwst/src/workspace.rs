use super::*;
use lib0::any::Any;
use serde::{ser::SerializeMap, Serialize, Serializer};
use yrs::{Map, PrelimMap, Transaction};

pub struct Workspace {
    id: String,
    blocks: Map,
    updated: Map,
}

impl Workspace {
    pub fn new<S: AsRef<str>>(trx: &mut Transaction, id: S) -> Self {
        let blocks = trx.get_map("blocks");

        // blocks.content
        let content = blocks
            .get("content")
            .or_else(|| {
                blocks.insert(trx, "content", PrelimMap::<Any>::new());
                blocks.get("content")
            })
            .and_then(|b| b.to_ymap())
            .unwrap();

        // blocks.updated
        let updated = blocks
            .get("updated")
            .or_else(|| {
                blocks.insert(trx, "updated", PrelimMap::<Any>::new());
                blocks.get("updated")
            })
            .and_then(|b| b.to_ymap())
            .unwrap();
        Self {
            id: id.as_ref().to_string(),
            blocks: content,
            updated,
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

    // create a block with specified flavor
    // if block exists, return the exists block
    pub fn create<B, F>(
        &self,
        trx: &mut Transaction,
        block_id: B,
        flavor: F,
        operator: u64,
    ) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        Block::new(self, trx, block_id, flavor, operator)
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S, operator: u64) -> Option<Block>
    where
        S: AsRef<str>,
    {
        Block::from(self, block_id, operator)
    }

    pub fn remove<S>(&self, trx: &mut Transaction, block_id: S) -> bool
    where
        S: AsRef<str>,
    {
        self.blocks.remove(trx, block_id.as_ref()).is_some()
            && self.updated().remove(trx, block_id.as_ref()).is_some()
    }

    pub fn exists<S>(&self, block_id: S) -> bool
    where
        S: AsRef<str>,
    {
        self.blocks.contains(block_id.as_ref())
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

#[cfg(test)]
mod test {
    use super::Workspace;
    use yrs::Doc;

    #[test]
    fn workspace() {
        let doc = Doc::default();
        let mut trx = doc.transact();

        let workspace = Workspace::new(&mut trx, "test");

        assert_eq!(workspace.id(), "test");
        assert_eq!(workspace.blocks().len(), 0);
        assert_eq!(workspace.updated().len(), 0);

        let block = workspace.create(&mut trx, "block", "text", 0);
        assert_eq!(workspace.blocks().len(), 1);
        assert_eq!(workspace.updated().len(), 1);
        assert_eq!(block.id(), "block");
        assert_eq!(block.flavor(), "text");

        assert_eq!(
            workspace.get("block", doc.client_id).map(|b| b.id()),
            Some("block".to_owned())
        );

        assert_eq!(workspace.exists("block"), true);

        assert_eq!(workspace.remove(&mut trx, "block"), true);
        assert_eq!(workspace.blocks().len(), 0);
        assert_eq!(workspace.updated().len(), 0);
        assert_eq!(workspace.get("block", doc.client_id), None);

        assert_eq!(workspace.exists("block"), false);
    }
}
