use lib0::any::Any;
use yrs::{Doc, Map, PrelimArray, PrelimMap, Transaction};

pub struct BlockField {
    r#type: String,
    default: String,
}

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
    fn new<S>(trx: &mut Transaction, block_id: S, flavor: S) -> Block
    where
        S: ToString,
    {
        let blocks = trx.get_map("blocks");

        // init base struct
        let block_id = block_id.to_string();
        blocks.insert(trx, block_id.clone(), PrelimMap::<Any>::new());
        let block = blocks.get(&block_id).and_then(|b| b.to_ymap()).unwrap();
        blocks.insert(trx, "content", PrelimMap::<Any>::new());
        let content = blocks.get(&block_id).and_then(|b| b.to_ymap()).unwrap();

        // init default schema
        content.insert(trx, "sys:flavor", flavor.to_string());
        content.insert(trx, "sys:version", PrelimArray::from([1, 0]));
        Self { block, content }
    }

    fn from(block: Map) -> Option<Block> {
        if let Some(content) = block.get("content").and_then(|b| b.to_ymap()) {
            Some(Self { block, content })
        } else {
            None
        }
    }

    // start with a namespace
    // for example: affine:text
    fn flavor(&self) -> String {
        self.content
            .get("sys:flavor")
            .unwrap_or_default()
            .to_string()
    }

    // block schema version
    // for example: [1, 0]
    fn version(&self) -> [usize; 2] {
        self.content
            .get("sys:version")
            .and_then(|v| {
                println!("{:?}", v);
                v.to_yarray()
            })
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
