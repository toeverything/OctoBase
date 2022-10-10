use super::*;
use lib0::any::Any;
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
    pub fn create<B, F, O>(
        &self,
        trx: &mut Transaction,
        block_id: B,
        flavor: F,
        operator: O,
    ) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
        O: TryInto<i64>,
    {
        Block::new(self, trx, block_id, flavor, operator)
    }

    // get a block if exists
    pub fn get<S, O>(&self, block_id: S, operator: O) -> Option<Block>
    where
        S: AsRef<str>,
        O: TryInto<i64>,
    {
        Block::from(self, block_id, operator)
    }
}
