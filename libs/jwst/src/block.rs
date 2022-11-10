use super::*;
use lib0::any::Any;
use serde::{Serialize, Serializer};
use std::{collections::HashMap, ops::RangeInclusive};
use yrs::{Array, Map, PrelimArray, PrelimMap, Transaction};

// The largest int in js number.
const MAX_JS_INT: i64 = 0x001F_FFFF_FFFF_FFFF;
// The smallest int in js number.
const MIN_JS_INT: i64 = -MAX_JS_INT;
const JS_INT_RANGE: RangeInclusive<i64> = MIN_JS_INT..=MAX_JS_INT;

#[derive(Debug, PartialEq)]
pub struct Block {
    // block schema
    // for example: {
    //     title: {type: 'string', default: ''}
    //     description: {type: 'string', default: ''}
    // }
    // default_props: HashMap<String, BlockField>,
    id: String,
    operator: u64,
    block: Map,
    children: Array,
    updated: Array,
}

unsafe impl Send for Block {}

impl Block {
    // Create a new block, skip create if block is already created.
    pub fn new<B, F>(
        workspace: &Workspace,
        trx: &mut Transaction,
        block_id: B,
        flavor: F,
        operator: u64,
    ) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        let block_id = block_id.as_ref();
        if let Some(block) = Self::from(workspace, block_id, operator) {
            block
        } else {
            // init base struct
            workspace
                .blocks()
                .insert(trx, block_id, PrelimMap::<Any>::new());
            let block = workspace
                .blocks()
                .get(block_id)
                .and_then(|b| b.to_ymap())
                .unwrap();

            // init default schema
            block.insert(trx, "sys:flavor", flavor.as_ref());
            block.insert(trx, "sys:version", PrelimArray::from([1, 0]));
            block.insert(
                trx,
                "sys:children",
                PrelimArray::<Vec<String>, String>::from(vec![]),
            );
            block.insert(
                trx,
                "sys:created",
                chrono::Utc::now().timestamp_millis() as f64,
            );

            workspace
                .updated()
                .insert(trx, block_id, PrelimArray::<_, Any>::from([]));

            trx.commit();

            let children = block
                .get("sys:children")
                .and_then(|c| c.to_yarray())
                .unwrap();
            let updated = workspace
                .updated()
                .get(block_id)
                .and_then(|c| c.to_yarray())
                .unwrap();

            let block = Self {
                id: block_id.to_string(),
                operator,
                block,
                children,
                updated,
            };

            block.log_update(trx, HistoryOperation::Add);

            block
        }
    }

    pub fn from<B>(workspace: &Workspace, block_id: B, operator: u64) -> Option<Block>
    where
        B: AsRef<str>,
    {
        let block = workspace.blocks().get(block_id.as_ref())?.to_ymap()?;
        let updated = workspace.updated().get(block_id.as_ref())?.to_yarray()?;

        let children = block.get("sys:children")?.to_yarray()?;
        Some(Self {
            id: block_id.as_ref().to_string(),
            operator,
            block,
            children,
            updated,
        })
    }

    pub fn from_raw_parts(id: String, block: Map, updated: Array, operator: u64) -> Block {
        let children = block.get("sys:children").unwrap().to_yarray().unwrap();
        Self {
            id,
            operator,
            block,
            children,
            updated,
        }
    }

    pub(crate) fn log_update(&self, trx: &mut Transaction, action: HistoryOperation) {
        let array = PrelimArray::from([
            Any::Number(self.operator as f64),
            Any::Number(chrono::Utc::now().timestamp_millis() as f64),
            Any::String(Box::from(action.to_string())),
        ]);

        self.updated.push_back(trx, array);
    }

    pub fn get(&self, key: &str) -> Option<Any> {
        let key = format!("prop:{}", key);
        self.block.get(&key).and_then(|v| match v.to_json() {
            Any::Null | Any::Undefined | Any::Array(_) | Any::Buffer(_) | Any::Map(_) => {
                error!("get wrong value at key {}", key);
                None
            }
            v => Some(v),
        })
    }

    pub fn set<T>(&self, trx: &mut Transaction, key: &str, value: T)
    where
        T: Into<Any>,
    {
        let key = format!("prop:{}", key);
        match value.into() {
            Any::Bool(bool) => {
                self.block.insert(trx, key, bool);
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::String(text) => {
                self.block.insert(trx, key, text.to_string());
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::Number(number) => {
                self.block.insert(trx, key, number);
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.block.insert(trx, key, number as f64);
                } else {
                    self.block.insert(trx, key, number);
                }
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::Null | Any::Undefined => {
                self.block.remove(trx, &key);
                self.log_update(trx, HistoryOperation::Delete);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
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

    pub fn created(&self) -> u64 {
        self.block
            .get("sys:created")
            .and_then(|c| match c.to_json() {
                Any::Number(n) => Some(n as u64),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn updated(&self) -> u64 {
        self.updated
            .iter()
            .filter_map(|v| v.to_yarray())
            .last()
            .and_then(|a| {
                a.get(1).and_then(|i| match i.to_json() {
                    Any::Number(n) => Some(n as u64),
                    _ => None,
                })
            })
            .unwrap_or_else(|| self.created())
    }

    pub fn history(&self) -> Vec<BlockHistory> {
        self.updated
            .iter()
            .filter_map(|v| v.to_yarray())
            .map(|v| (v, self.id.clone()).into())
            .collect()
    }

    pub fn parent(&self) -> Option<String> {
        self.block
            .get("sys:parent")
            .and_then(|c| match c.to_json() {
                Any::String(s) => Some(s.to_string()),
                _ => None,
            })
    }

    pub fn children(&self) -> Vec<String> {
        self.children.iter().map(|v| v.to_string()).collect()
    }

    #[inline]
    pub fn children_iter(&self) -> impl Iterator<Item = String> + '_ {
        self.children.iter().map(|v| v.to_string())
    }

    pub fn children_len(&self) -> u32 {
        self.children.len()
    }

    pub fn content(&self) -> HashMap<String, Any> {
        self.block
            .iter()
            .filter_map(|(key, val)| {
                if key.starts_with("prop:") {
                    Some((key[5..].to_owned(), val.to_json()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn set_parent(&self, trx: &mut Transaction, block_id: String) {
        self.block.insert(trx, "sys:parent", block_id);
    }

    pub fn push_children(&self, trx: &mut Transaction, block: &Block) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        self.children.push_back(trx, block.id.clone());

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn insert_children_at(&self, trx: &mut Transaction, block: &Block, pos: u32) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        let children = &self.children;

        if children.len() > pos {
            children.insert(trx, pos, block.id.clone());
        } else {
            children.push_back(trx, block.id.clone());
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn insert_children_before(&self, trx: &mut Transaction, block: &Block, reference: &str) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        let children = &self.children;

        if let Some(pos) = children.iter().position(|c| c.to_string() == reference) {
            children.insert(trx, pos as u32, block.id.clone());
        } else {
            children.push_back(trx, block.id.clone());
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn insert_children_after(&self, trx: &mut Transaction, block: &Block, reference: &str) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        let children = &self.children;

        match children.iter().position(|c| c.to_string() == reference) {
            Some(pos) if (pos as u32) < children.len() => {
                children.insert(trx, pos as u32 + 1, block.id.clone());
            }
            _ => {
                children.push_back(trx, block.id.clone());
            }
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn remove_children(&self, trx: &mut Transaction, block: &Block) {
        let children = &self.children;
        block.set_parent(trx, self.id.clone());

        if let Some(current_pos) = children.iter().position(|c| c.to_string() == block.id) {
            children.remove(trx, current_pos as u32);
            self.log_update(trx, HistoryOperation::Delete);
        }
    }

    pub fn exists_children(&self, block_id: &str) -> Option<usize> {
        self.children.iter().position(|c| c.to_string() == block_id)
    }
}

impl Serialize for Block {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let any = self.block.to_json();
        any.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn init_block() {
        let workspace = Workspace::new("test");

        // new block
        let block = workspace.get_trx().create("test", "affine:text");
        assert_eq!(block.id(), "test");
        assert_eq!(block.flavor(), "affine:text");
        assert_eq!(block.version(), [1, 0]);

        // get exist block
        let block = workspace.get("test").unwrap();
        assert_eq!(block.flavor(), "affine:text");
        assert_eq!(block.version(), [1, 0]);
    }

    #[test]
    fn set_value() {
        let workspace = Workspace::new("test");

        let block = workspace.with_trx(|mut t| {
            let block = t.create("test", "affine:text");

            let trx = &mut t.trx;

            // normal type set
            block.set(trx, "bool", true);
            block.set(trx, "text", "hello world");
            block.set(trx, "text_owned", "hello world".to_owned());
            block.set(trx, "num", 123_i64);
            block.set(trx, "bigint", 9007199254740992_i64);

            block
        });

        assert_eq!(block.get("bool").unwrap().to_string(), "true");
        assert_eq!(block.get("text").unwrap().to_string(), "hello world");
        assert_eq!(block.get("text_owned").unwrap().to_string(), "hello world");
        assert_eq!(block.get("num").unwrap().to_string(), "123");
        assert_eq!(block.get("bigint").unwrap().to_string(), "9007199254740992");

        assert_eq!(
            block.content(),
            vec![
                ("bool".to_owned(), Any::Bool(true)),
                ("text".to_owned(), Any::String("hello world".into())),
                ("text_owned".to_owned(), Any::String("hello world".into())),
                ("num".to_owned(), Any::Number(123.0)),
                ("bigint".to_owned(), Any::BigInt(9007199254740992)),
            ]
            .iter()
            .cloned()
            .collect::<HashMap<_, _>>()
        );
    }

    #[test]
    fn insert_remove_children() {
        let workspace = Workspace::new("text");

        workspace.with_trx(|mut t| {
            let block = t.create("a", "affine:text");
            let b = t.create("b", "affine:text");
            let c = t.create("c", "affine:text");
            let d = t.create("d", "affine:text");
            let e = t.create("e", "affine:text");
            let f = t.create("f", "affine:text");
            let trx = &mut t.trx;

            block.push_children(trx, &b);
            block.insert_children_at(trx, &c, 0);
            block.insert_children_before(trx, &d, "b");
            block.insert_children_after(trx, &e, "b");
            block.insert_children_after(trx, &f, "c");

            assert_eq!(
                block.children(),
                vec![
                    "c".to_owned(),
                    "f".to_owned(),
                    "d".to_owned(),
                    "b".to_owned(),
                    "e".to_owned()
                ]
            );

            block.remove_children(trx, &d);

            assert_eq!(
                block
                    .children
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>(),
                vec![
                    "c".to_owned(),
                    "f".to_owned(),
                    "b".to_owned(),
                    "e".to_owned()
                ]
            );
        });
    }

    #[test]
    fn updated() {
        let workspace = Workspace::new("test");
        let block = workspace.with_trx(|mut t| {
            let block = t.create("a", "affine:text");
            block.set(&mut t.trx, "test", 1);

            block
        });

        assert!(block.created() <= block.updated())
    }

    #[test]
    fn history() {
        use yrs::Doc;

        let doc = Doc::with_client_id(123);

        let workspace = Workspace::from_doc(doc, "test");

        let block = workspace.with_trx(|mut t| {
            let block = t.create("a", "affine:text");
            let b = t.create("b", "affine:text");
            let trx = &mut t.trx;
            block.set(trx, "test", 1);

            let history = block.history();

            assert_eq!(history.len(), 2);

            // let history = history.last().unwrap();

            assert_eq!(
                history,
                vec![
                    BlockHistory {
                        block_id: "a".to_owned(),
                        client: 123,
                        timestamp: history.get(0).unwrap().timestamp,
                        operation: HistoryOperation::Add,
                    },
                    BlockHistory {
                        block_id: "a".to_owned(),
                        client: 123,
                        timestamp: history.get(1).unwrap().timestamp,
                        operation: HistoryOperation::Update,
                    }
                ]
            );

            block.push_children(trx, &b);

            assert_eq!(block.exists_children("b"), Some(0));

            block.remove_children(trx, &b);

            assert_eq!(block.exists_children("b"), None);

            block
        });

        let history = block.history();
        assert_eq!(history.len(), 4);

        if let [.., insert, remove] = history.as_slice() {
            assert_eq!(
                insert,
                &BlockHistory {
                    block_id: "a".to_owned(),
                    client: 123,
                    timestamp: insert.timestamp,
                    operation: HistoryOperation::Add,
                }
            );
            assert_eq!(
                remove,
                &BlockHistory {
                    block_id: "a".to_owned(),
                    client: 123,
                    timestamp: remove.timestamp,
                    operation: HistoryOperation::Delete,
                }
            );
        } else {
            assert!(false)
        }
    }
}
