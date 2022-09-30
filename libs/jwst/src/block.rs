use super::*;
use lib0::any::Any;
use types::{BlockContentValue, BlockHistory, JsonValue};
use yrs::{Array, Map, PrelimArray, PrelimMap, Transaction};
struct BlockChildrenPosition {
    pos: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

impl From<&InsertChildren> for RemoveChildren {
    fn from(options: &InsertChildren) -> RemoveChildren {
        Self {
            block_id: options.block_id.clone(),
        }
    }
}

impl From<&InsertChildren> for BlockChildrenPosition {
    fn from(options: &InsertChildren) -> BlockChildrenPosition {
        Self {
            pos: options.pos,
            before: options.before.clone(),
            after: options.after.clone(),
        }
    }
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
    operator: i64,
    block: Map,
    children: Array,
    content: Map,
    updated: Array,
}

impl Block {
    fn get_root(trx: &mut Transaction) -> (Map, Map) {
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
        (content, updated)
    }

    // Create a new block, skip create if block is already created.
    pub fn new<B, F, O>(trx: &mut Transaction, block_id: B, flavor: F, operator: O) -> Block
    where
        B: ToString,
        F: ToString,
        O: TryInto<i64>,
    {
        let block_id = block_id.to_string();
        let operator = operator.try_into().unwrap_or_default();
        if let Some(block) = Self::from(trx, &block_id, operator) {
            block
        } else {
            let (blocks, updated) = Self::get_root(trx);

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
            block.insert(trx, "sys:created", chrono::Utc::now().timestamp_millis());
            block.insert(trx, "content", PrelimMap::<Any>::new());

            updated.insert(trx, block_id.clone(), PrelimArray::<_, Any>::from([]));

            trx.commit();

            let children = block
                .get("sys:children")
                .and_then(|c| c.to_yarray())
                .unwrap();
            let content = block.get("content").and_then(|c| c.to_ymap()).unwrap();
            let updated = updated.get(&block_id).and_then(|c| c.to_yarray()).unwrap();

            Self {
                id: block_id.clone(),
                operator,
                block,
                children,
                content,
                updated,
            }
        }
    }

    pub fn from<B, O>(trx: &mut Transaction, block_id: B, operator: O) -> Option<Block>
    where
        B: ToString,
        O: TryInto<i64>,
    {
        let (blocks, updated) = Self::get_root(trx);

        let block_id = block_id.to_string();

        blocks
            .get(&block_id)
            .and_then(|b| b.to_ymap())
            .and_then(|block| {
                updated
                    .get(&block_id)
                    .and_then(|u| u.to_yarray())
                    .map(|updated| (block, updated))
            })
            .and_then(|(block, updated)| {
                if let (Some(children), Some(content)) = (
                    block.get("sys:children").and_then(|c| c.to_yarray()),
                    block.get("content").and_then(|c| c.to_ymap()),
                ) {
                    Some(Self {
                        id: block_id,
                        operator: operator.try_into().unwrap_or_default(),
                        block,
                        children,
                        content,
                        updated,
                    })
                } else {
                    None
                }
            })
    }

    pub fn block(&self) -> &Map {
        &self.block
    }

    pub(crate) fn content(&mut self) -> &mut Map {
        &mut self.content
    }

    fn log_update(&self, trx: &mut Transaction) {
        let array = PrelimArray::from([self.operator, chrono::Utc::now().timestamp_millis()]);
        self.updated.push_back(trx, array);
    }

    pub(self) fn set_value(
        block: &mut Map,
        trx: &mut Transaction,
        key: &str,
        value: JsonValue,
    ) -> bool {
        match value {
            JsonValue::Bool(v) => {
                block.insert(trx, key.clone(), v);
                true
            }
            JsonValue::Null => {
                block.remove(trx, key);
                true
            }
            JsonValue::Number(v) => {
                if let Some(v) = v.as_f64() {
                    block.insert(trx, key.clone(), v);
                    true
                } else if let Some(v) = v.as_i64() {
                    block.insert(trx, key.clone(), v);
                    true
                } else if let Some(v) = v.as_u64() {
                    block.insert(trx, key.clone(), i64::try_from(v).unwrap_or(0));
                    true
                } else {
                    false
                }
            }
            JsonValue::String(v) => {
                block.insert(trx, key.clone(), v.clone());
                true
            }
            _ => false,
        }
    }

    pub fn set<T>(&mut self, trx: &mut Transaction, key: &str, value: T)
    where
        T: Into<BlockContentValue>,
    {
        match value.into() {
            BlockContentValue::Json(json) => {
                if Self::set_value(self.content(), trx, key, json) {
                    self.log_update(trx);
                }
            }
            BlockContentValue::Boolean(bool) => {
                self.content.insert(trx, key, bool);
                self.log_update(trx);
            }
            BlockContentValue::Text(text) => {
                self.content.insert(trx, key, text);
                self.log_update(trx);
            }
            BlockContentValue::Number(number) => {
                self.content.insert(trx, key, number);
                self.log_update(trx);
            }
        }
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

    pub fn updated(&self) -> i64 {
        self.updated
            .iter()
            .filter_map(|v| v.to_yarray())
            .last()
            .and_then(|a| a.get(1).and_then(|i| i.to_string().parse::<i64>().ok()))
            .unwrap_or_else(|| {
                self.block
                    .get("sys:flavor")
                    .and_then(|i| i.to_string().parse::<i64>().ok())
                    .unwrap_or_default()
            })
    }

    pub fn history(&self) -> Vec<[i64; 2]> {
        self.updated
            .iter()
            .filter_map(|v| v.to_yarray())
            .map(|v| {
                v.iter()
                    .take(2)
                    .filter_map(|s| s.to_string().parse::<i64>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|v| v.len() == 2)
            .map(|v| v.try_into().unwrap())
            .collect()
    }

    pub fn insert_children(&mut self, trx: &mut Transaction, options: InsertChildren) {
        self.remove_children(trx, (&options).into());

        let children = &self.children;

        if let Some(position) = self.position_calculator(children.len(), (&options).into()) {
            children.insert(trx, position, options.block_id);
        } else {
            children.push_back(trx, options.block_id);
        }

        self.log_update(trx);
    }

    pub fn remove_children(&mut self, trx: &mut Transaction, options: RemoveChildren) {
        let children = &self.children;

        if let Some(current_pos) = children
            .iter()
            .position(|c| c.to_string() == options.block_id)
        {
            children.remove(trx, current_pos as u32)
        }
        self.log_update(trx);
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

    #[test]
    fn init_block() {
        use super::Block;
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let block = Block::new(&mut trx, "test", "affine:text", 123);

        debug_assert!(block.flavor() == "affine:text");
        debug_assert!(block.version() == [1, 0]);
    }

    #[test]
    fn set_value() {
        use super::Block;
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let mut block = Block::new(&mut trx, "test", "affine:text", doc.client_id);
        block.set(&mut trx, "bool", true);
        block.set(&mut trx, "text", "hello world");
        block.set(&mut trx, "num", 123);
        debug_assert!(block.content().get("bool").unwrap().to_string() == "true");
        debug_assert!(block.content().get("text").unwrap().to_string() == "hello world");
        debug_assert!(block.content().get("num").unwrap().to_string() == "123");
    }

    #[test]
    fn insert_remove_children() {
        use super::{Block, InsertChildren, RemoveChildren};
        use yrs::Doc;

        let doc = Doc::default();
        let mut trx = doc.transact();

        let mut block = Block::new(&mut trx, "a", "affine:text", 123);

        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "b".to_owned(),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "c".to_owned(),
                pos: Some(0),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
                block_id: "d".to_owned(),
                before: Some("b".to_owned()),
                ..Default::default()
            },
        );
        block.insert_children(
            &mut trx,
            InsertChildren {
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
        );

        block.remove_children(
            &mut trx,
            RemoveChildren {
                block_id: "d".to_owned(),
            },
        );

        debug_assert!(
            block
                .children
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                == vec!["c".to_owned(), "b".to_owned(), "e".to_owned()]
        );
    }
}
