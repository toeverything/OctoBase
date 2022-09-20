use lib0::any::Any;
use serde::Deserialize;
use utoipa::ToSchema;
use yrs::{block, Array, Doc, Map, PrelimArray, PrelimMap, Transaction};

pub struct BlockField {
    r#type: String,
    default: String,
}

struct BlockChildrenPosition {
    pos: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Default, Deserialize, ToSchema)]
#[schema(example = json!({"block_id": "jwstRf4rMzua7E", "pos": 0}))]
pub struct InsertBlock {
    pub block_id: String,
    pos: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug)]
pub struct Block {
    // block schema
    // for example: {
    //     title: {type: 'string', default: ''}
    //     description: {type: 'string', default: ''}
    // }
    // default_props: HashMap<String, BlockField>,
    id: String,
    block: Map,
    children: Array,
    content: Map,
}

impl Block {
    fn get_root(trx: &mut Transaction) -> Map {
        let blocks = trx.get_map("blocks");
        blocks
            .get("content")
            .or_else(|| {
                blocks.insert(trx, "content", PrelimMap::<Any>::new());
                blocks.get("content")
            })
            .and_then(|b| b.to_ymap())
            .unwrap()
    }

    // Create a new block, skip create if block is already created.
    pub fn new<B: ToString, F: ToString>(trx: &mut Transaction, block_id: B, flavor: F) -> Block {
        let block_id = block_id.to_string();
        if let Some(block) = Self::from(trx, &block_id) {
            block
        } else {
            let blocks = Self::get_root(trx);

            // init base struct
            blocks.insert(trx, block_id.clone(), PrelimMap::<Any>::new());
            let block = blocks.get(&block_id).and_then(|b| b.to_ymap()).unwrap();

            // init default schema
            block.insert(trx, "sys:flavor", flavor.to_string());
            block.insert(trx, "sys:version", PrelimArray::from([1, 0]));
            block.insert(
                trx,
                "sys:children",
                PrelimArray::<Vec<String>, String>::from(vec![]),
            );
            block.insert(trx, "content", PrelimMap::<Any>::new());

            trx.commit();

            let children = block
                .get("sys:children")
                .and_then(|c| c.to_yarray())
                .unwrap();
            let content = block.get("content").and_then(|c| c.to_ymap()).unwrap();

            Self {
                id: block_id.clone(),
                block,
                children,
                content,
            }
        }
    }

    pub fn from<B: ToString>(trx: &mut Transaction, block_id: B) -> Option<Block> {
        let blocks = Self::get_root(trx);

        let block_id = block_id.to_string();

        blocks
            .get(&block_id)
            .and_then(|b| b.to_ymap())
            .and_then(|block| {
                if let (Some(children), Some(content)) = (
                    block.get("sys:children").and_then(|c| c.to_yarray()),
                    block.get("content").and_then(|c| c.to_ymap()),
                ) {
                    Some(Self {
                        id: block_id,
                        block,
                        children,
                        content,
                    })
                } else {
                    None
                }
            })
    }

    pub fn block(&self) -> &Map {
        &self.block
    }

    pub fn content(&mut self) -> &mut Map {
        &mut self.content
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    // start with a namespace
    // for example: affine:text
    pub fn flavor(&self) -> String {
        self.block.get("sys:flavor").unwrap_or_default().to_string()
    }

    // block schema version
    // for example: [1, 0]
    pub fn version(&self) -> [usize; 2] {
        self.block
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

    pub fn insert_children(&mut self, trx: &mut Transaction, options: InsertBlock) {
        let children = &self.children;

        if let Some(current_pos) = children
            .iter()
            .position(|c| c.to_string() == options.block_id)
        {
            children.remove(trx, current_pos as u32)
        }

        if let Some(position) = self.position_calculator(
            children.len(),
            BlockChildrenPosition {
                pos: options.pos,
                before: options.before,
                after: options.after,
            },
        ) {
            children.insert(trx, position, options.block_id);
        } else {
            children.push_back(trx, options.block_id);
        }

        // this._setBlock(block.id, content);
    }

    fn position_calculator(&self, max_pos: u32, position: BlockChildrenPosition) -> Option<u32> {
        let BlockChildrenPosition { pos, before, after } = position;
        let children = &self.children;
        if let Some(pos) = pos {
            if pos < max_pos {
                return Some(pos);
            }
        } else if let Some(before) = before {
            if let Some(current_pos) = children.iter().position(|c| c.to_string() == before) {
                let prev_pos = current_pos as u32;
                if prev_pos < max_pos {
                    return Some(prev_pos);
                }
            }
        } else if let Some(after) = after {
            if let Some(current_pos) = children.iter().position(|c| c.to_string() == after) {
                let next_pos = current_pos as u32 + 1;
                if next_pos < max_pos {
                    return Some(next_pos);
                }
            }
        }
        None
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

    #[test]
    fn insert_block() {
        let doc = Doc::default();
        let mut trx = doc.transact();

        let mut block = Block::new(&mut trx, "a", "affine:text");

        block.insert_children(
            &mut trx,
            InsertBlock {
                block_id: "b".to_owned(),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertBlock {
                block_id: "c".to_owned(),
                pos: Some(0),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertBlock {
                block_id: "d".to_owned(),
                before: Some("b".to_owned()),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertBlock {
                block_id: "e".to_owned(),
                after: Some("b".to_owned()),
                ..Default::default()
            },
        );

        debug_assert!(
            block
                .children
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                == vec![
                    "c".to_owned(),
                    "d".to_owned(),
                    "b".to_owned(),
                    "e".to_owned()
                ]
        )
    }
}
