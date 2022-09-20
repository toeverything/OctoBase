use lib0::any::Any;
use yrs::{Doc, Map, PrelimArray, PrelimMap, Transaction};

pub struct BlockField {
    r#type: String,
    default: String,
}

#[derive(Debug)]
pub struct Block {
    // block schema
    // for example: {
    //     title: {type: 'string', default: ''}
    //     description: {type: 'string', default: ''}
    // }
    // default_props: HashMap<String, BlockField>,
    block: Map,
    content: Map,
}

impl Block {
    // Create a new block, skip create if block is already created.
    pub fn new<B, F>(trx: &mut Transaction, block_id: B, flavor: F) -> Block
    where
        B: ToString,
        F: ToString,
    {
        let blocks = trx.get_map("blocks");
        let blocks = blocks
            .get("content")
            .or_else(|| blocks.insert(trx, "content", PrelimMap::<Any>::new()))
            .and_then(|b| b.to_ymap())
            .unwrap();

        // init base struct
        let block_id = block_id.to_string();

        if let Some((block, content)) =
            blocks
                .get(&block_id)
                .and_then(|b| b.to_ymap())
                .and_then(|block| {
                    block
                        .get("content")
                        .and_then(|c| c.to_ymap())
                        .map(|content| (block, content))
                })
        {
            Self { block, content }
        } else {
            blocks.insert(trx, block_id.clone(), PrelimMap::<Any>::new());
            let block = blocks.get(&block_id).and_then(|b| b.to_ymap()).unwrap();

            // init default schema
            block.insert(trx, "sys:flavor", flavor.to_string());
            block.insert(trx, "sys:version", PrelimArray::from([1, 0]));
            block.insert(trx, "content", PrelimMap::<Any>::new());

            trx.commit();

            let content = block.get("content").and_then(|c| c.to_ymap()).unwrap();

            Self { block, content }
        }
    }

    pub fn from(block: Map) -> Option<Block> {
        if let Some(content) = block.get("content").and_then(|b| b.to_ymap()) {
            Some(Self { block, content })
        } else {
            None
        }
    }

    pub fn block(&self) -> &Map {
        &self.block
    }

    pub fn content(&mut self) -> &mut Map {
        &mut self.content
    }

    // start with a namespace
    // for example: affine:text
    pub fn flavor(&self) -> String {
        self.content
            .get("sys:flavor")
            .unwrap_or_default()
            .to_string()
    }

    // block schema version
    // for example: [1, 0]
    pub fn version(&self) -> [usize; 2] {
        self.content
            .get("sys:version")
            .and_then(|v| v.to_yarray())
            .map(|v| {
                v.iter()
                    .take(2)
                    .filter_map(|s| s.to_string().parse::<usize>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap()
            .try_into()
            .unwrap()
    }
}

mod tests {
    use super::*;

    #[test]
    fn init_block() {
        let doc = Doc::default();
        let mut trx = doc.transact();

        let block = Block::new(&mut trx, "test", "affine:text");

        debug_assert!(block.flavor() == "affine:text");
        debug_assert!(block.version() == [1, 0]);
    }
}
