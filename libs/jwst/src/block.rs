use super::{constants::sys, utils::JS_INT_RANGE, *};
use lib0::any::Any;
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use yrs::{
    types::ToJson, Array, ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, ReadTxn, Transact,
    TransactionMut,
};

#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    // block schema
    // for example: {
    //     title: {type: 'string', default: ''}
    //     description: {type: 'string', default: ''}
    // }
    // default_props: HashMap<String, BlockField>,
    id: String,
    doc: Doc,
    operator: u64,
    block: MapRef,
    children: ArrayRef,
    updated: ArrayRef,
}

unsafe impl Send for Block {}

impl Block {
    // Create a new block, skip create if block is already created.
    pub fn new<B, F>(
        trx: &mut TransactionMut<'_>,
        workspace: &Workspace,
        block_id: B,
        flavor: F,
        operator: u64,
    ) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        let block_id = block_id.as_ref();

        if let Some(block) = Self::from(trx, workspace, block_id, operator) {
            block
        } else {
            // init base struct
            workspace
                .blocks
                .insert(trx, block_id, MapPrelim::<Any>::new());
            let block = workspace
                .blocks
                .get(trx, block_id)
                .and_then(|b| b.to_ymap())
                .unwrap();

            // init default schema
            block.insert(trx, sys::FLAVOR, flavor.as_ref());
            block.insert(trx, sys::VERSION, ArrayPrelim::from([1, 0]));
            block.insert(
                trx,
                sys::CHILDREN,
                ArrayPrelim::<Vec<String>, String>::from(vec![]),
            );
            block.insert(
                trx,
                sys::CREATED,
                chrono::Utc::now().timestamp_millis() as f64,
            );

            workspace
                .updated
                .insert(trx, block_id, ArrayPrelim::<_, Any>::from([]));

            let children = block
                .get(trx, sys::CHILDREN)
                .and_then(|c| c.to_yarray())
                .unwrap();
            let updated = workspace
                .updated
                .get(trx, block_id)
                .and_then(|c| c.to_yarray())
                .unwrap();

            let block = Self {
                doc: workspace.doc(),
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

    pub fn from<T, B>(trx: &T, workspace: &Workspace, block_id: B, operator: u64) -> Option<Block>
    where
        T: ReadTxn,
        B: AsRef<str>,
    {
        let block = workspace.blocks.get(trx, block_id.as_ref())?.to_ymap()?;
        let updated = workspace.updated.get(trx, block_id.as_ref())?.to_yarray()?;

        let children = block.get(trx, sys::CHILDREN)?.to_yarray()?;

        Some(Self {
            id: block_id.as_ref().to_string(),
            doc: workspace.doc(),
            operator,
            block,
            children,
            updated,
        })
    }

    pub fn from_raw_parts<T: ReadTxn>(
        trx: &T,
        id: String,
        doc: &Doc,
        block: MapRef,
        updated: ArrayRef,
        operator: u64,
    ) -> Block {
        let children = block.get(trx, sys::CHILDREN).unwrap().to_yarray().unwrap();
        Self {
            id,
            doc: doc.clone(),
            operator,
            block,
            children,
            updated,
        }
    }

    pub(crate) fn log_update(&self, trx: &mut TransactionMut, action: HistoryOperation) {
        let array = ArrayPrelim::from([
            Any::Number(self.operator as f64),
            Any::Number(chrono::Utc::now().timestamp_millis() as f64),
            Any::String(Box::from(action.to_string())),
        ]);

        self.updated.push_back(trx, array);
    }

    pub fn get<T>(&self, trx: &T, key: &str) -> Option<Any>
    where
        T: ReadTxn,
    {
        let key = format!("prop:{key}");
        self.block
            .get(trx, &key)
            .and_then(|v| match v.to_json(trx) {
                Any::Null | Any::Undefined | Any::Array(_) | Any::Buffer(_) | Any::Map(_) => {
                    error!("get wrong value at key {}", key);
                    None
                }
                v => Some(v),
            })
    }

    pub fn set<T>(&self, trx: &mut TransactionMut, key: &str, value: T)
    where
        T: Into<Any>,
    {
        let key = format!("prop:{key}");
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
    pub fn flavor<T>(&self, trx: &T) -> String
    where
        T: ReadTxn,
    {
        self.block
            .get(trx, sys::FLAVOR)
            .unwrap_or_default()
            .to_string(trx)
    }

    // block schema version
    // for example: [1, 0]
    pub fn version<T>(&self, trx: &T) -> [usize; 2]
    where
        T: ReadTxn,
    {
        self.block
            .get(trx, sys::VERSION)
            .and_then(|v| v.to_yarray())
            .map(|v| {
                v.iter(trx)
                    .take(2)
                    .filter_map(|s| s.to_string(trx).parse::<usize>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap()
            .try_into()
            .unwrap()
    }

    pub fn created<T>(&self, trx: &T) -> u64
    where
        T: ReadTxn,
    {
        self.block
            .get(trx, sys::CREATED)
            .and_then(|c| match c.to_json(trx) {
                Any::Number(n) => Some(n as u64),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn updated<T>(&self, trx: &T) -> u64
    where
        T: ReadTxn,
    {
        self.updated
            .iter(trx)
            .filter_map(|v| v.to_yarray())
            .last()
            .and_then(|a| {
                a.get(trx, 1).and_then(|i| match i.to_json(trx) {
                    Any::Number(n) => Some(n as u64),
                    _ => None,
                })
            })
            .unwrap_or_else(|| self.created(trx))
    }

    pub fn history<T>(&self, trx: &T) -> Vec<BlockHistory>
    where
        T: ReadTxn,
    {
        self.updated
            .iter(trx)
            .filter_map(|v| v.to_yarray())
            .map(|v| (trx, v, self.id.clone()).into())
            .collect()
    }

    pub fn parent<T>(&self, trx: &T) -> Option<String>
    where
        T: ReadTxn,
    {
        self.block
            .get(trx, sys::PARENT)
            .and_then(|c| match c.to_json(trx) {
                Any::String(s) => Some(s.to_string()),
                _ => None,
            })
    }

    pub fn children<T>(&self, trx: &T) -> Vec<String>
    where
        T: ReadTxn,
    {
        self.children.iter(trx).map(|v| v.to_string(trx)).collect()
    }

    #[inline]
    pub fn children_iter<T>(&self, cb: impl Fn(Box<dyn Iterator<Item = String> + '_>) -> T) -> T {
        let trx = self.doc.transact();
        let iterator = self.children.iter(&trx).map(|v| v.to_string(&trx));

        cb(Box::new(iterator))
    }

    pub fn children_len(&self) -> u32 {
        let trx = self.doc.transact();
        self.children.len(&trx)
    }

    pub fn children_exists<T, S>(&self, trx: &T, block_id: S) -> bool
    where
        T: ReadTxn,
        S: AsRef<str>,
    {
        self.children
            .iter(trx)
            .map(|v| v.to_string(trx))
            .any(|bid| bid == block_id.as_ref())
    }

    pub(crate) fn content<T>(&self, trx: &T) -> HashMap<String, Any>
    where
        T: ReadTxn,
    {
        self.block
            .iter(trx)
            .filter_map(|(key, val)| {
                key.strip_prefix("prop:")
                    .map(|stripped| (stripped.to_owned(), val.to_json(trx)))
            })
            .collect()
    }

    fn set_parent(&self, trx: &mut TransactionMut, block_id: String) {
        self.block.insert(trx, sys::PARENT, block_id);
    }

    pub fn push_children(&self, trx: &mut TransactionMut, block: &Block) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        self.children.push_back(trx, block.id.clone());

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn insert_children_at(&self, trx: &mut TransactionMut, block: &Block, pos: u32) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        let children = &self.children;

        if children.len(trx) > pos {
            children.insert(trx, pos, block.id.clone());
        } else {
            children.push_back(trx, block.id.clone());
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn insert_children_before(&self, trx: &mut TransactionMut, block: &Block, reference: &str) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        let children = &self.children;

        if let Some(pos) = children
            .iter(trx)
            .position(|c| c.to_string(trx) == reference)
        {
            children.insert(trx, pos as u32, block.id.clone());
        } else {
            children.push_back(trx, block.id.clone());
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn insert_children_after(&self, trx: &mut TransactionMut, block: &Block, reference: &str) {
        self.remove_children(trx, block);
        block.set_parent(trx, self.id.clone());

        let children = &self.children;

        match children
            .iter(trx)
            .position(|c| c.to_string(trx) == reference)
        {
            Some(pos) if (pos as u32) < children.len(trx) => {
                children.insert(trx, pos as u32 + 1, block.id.clone());
            }
            _ => {
                children.push_back(trx, block.id.clone());
            }
        }

        self.log_update(trx, HistoryOperation::Add);
    }

    pub fn remove_children(&self, trx: &mut TransactionMut, block: &Block) {
        let children = &self.children;
        block.set_parent(trx, self.id.clone());

        if let Some(current_pos) = children
            .iter(trx)
            .position(|c| c.to_string(trx) == block.id)
        {
            children.remove(trx, current_pos as u32);
            self.log_update(trx, HistoryOperation::Delete);
        }
    }

    pub fn exists_children<T>(&self, trx: &T, block_id: &str) -> Option<usize>
    where
        T: ReadTxn,
    {
        self.children
            .iter(trx)
            .position(|c| c.to_string(trx) == block_id)
    }
}

impl Serialize for Block {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let trx = self.doc.transact();
        let any = self.block.to_json(&trx);
        any.serialize(serializer)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn init_block() {
        let workspace = Workspace::new("test");

        // new block
        workspace.with_trx(|mut t| {
            let block = t.create("test", "affine:text");

            assert_eq!(block.id(), "test");
            assert_eq!(block.flavor(&t.trx), "affine:text");
            assert_eq!(block.version(&t.trx), [1, 0]);
        });

        // get exist block
        workspace.with_trx(|t| {
            let block = workspace.get(&t.trx, "test").unwrap();

            assert_eq!(block.flavor(&t.trx), "affine:text");
            assert_eq!(block.version(&t.trx), [1, 0]);
        });
    }

    #[test]
    fn set_value() {
        let workspace = Workspace::new("test");

        workspace.with_trx(|mut t| {
            let block = t.create("test", "affine:text");

            // normal type set
            block.set(&mut t.trx, "bool", true);
            block.set(&mut t.trx, "text", "hello world");
            block.set(&mut t.trx, "text_owned", "hello world".to_owned());
            block.set(&mut t.trx, "num", 123_i64);
            block.set(&mut t.trx, "bigint", 9007199254740992_i64);

            assert_eq!(block.get(&t.trx, "bool").unwrap().to_string(), "true");
            assert_eq!(
                block.get(&t.trx, "text").unwrap().to_string(),
                "hello world"
            );
            assert_eq!(
                block.get(&t.trx, "text_owned").unwrap().to_string(),
                "hello world"
            );
            assert_eq!(block.get(&t.trx, "num").unwrap().to_string(), "123");
            assert_eq!(
                block.get(&t.trx, "bigint").unwrap().to_string(),
                "9007199254740992"
            );

            assert_eq!(
                block.content(&t.trx),
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
        });
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

            block.push_children(&mut t.trx, &b);
            block.insert_children_at(&mut t.trx, &c, 0);
            block.insert_children_before(&mut t.trx, &d, "b");
            block.insert_children_after(&mut t.trx, &e, "b");
            block.insert_children_after(&mut t.trx, &f, "c");

            assert_eq!(
                block.children(&t.trx),
                vec![
                    "c".to_owned(),
                    "f".to_owned(),
                    "d".to_owned(),
                    "b".to_owned(),
                    "e".to_owned()
                ]
            );

            block.remove_children(&mut t.trx, &d);

            assert_eq!(
                block.children(&t.trx),
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

        workspace.with_trx(|mut t| {
            let block = t.create("a", "affine:text");

            block.set(&mut t.trx, "test", 1);

            assert!(block.created(&t.trx) <= block.updated(&t.trx))
        });
    }

    #[test]
    fn history() {
        use yrs::Doc;

        let doc = Doc::with_client_id(123);

        let workspace = Workspace::from_doc(doc, "test");

        let (block, b, history) = workspace.with_trx(|mut t| {
            let block = t.create("a", "affine:text");
            let b = t.create("b", "affine:text");

            block.set(&mut t.trx, "test", 1);

            let history = block.history(&t.trx);

            (block, b, history)
        });

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

        let history = workspace.with_trx(|mut t| {
            block.push_children(&mut t.trx, &b);

            assert_eq!(block.exists_children(&t.trx, "b"), Some(0));

            block.remove_children(&mut t.trx, &b);

            assert_eq!(block.exists_children(&t.trx, "b"), None);

            block.history(&t.trx)
        });

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
