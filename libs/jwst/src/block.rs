use super::{constants::sys, utils::JS_INT_RANGE, *};
use lib0::any::Any;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;
use yrs::{
    types::ToJson, Array, ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, ReadTxn, Transact,
    TransactionMut,
};

#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    id: String,
    space_id: String,
    block_id: String,
    doc: Doc,
    operator: u64,
    block: MapRef,
    children: ArrayRef,
    updated: Option<ArrayRef>,
}

unsafe impl Send for Block {}

impl Block {
    // Create a new block, skip create if block is already created.
    pub fn new<B, F>(
        trx: &mut TransactionMut<'_>,
        space: &Space,
        block_id: B,
        flavor: F,
        operator: u64,
    ) -> JwstResult<Block>
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        let block_id = block_id.as_ref();

        if let Some(block) = Self::from(trx, space, block_id, operator) {
            Ok(block)
        } else {
            // init base struct
            space
                .blocks
                .insert(trx, block_id, MapPrelim::<Any>::new())?;
            let block = space
                .blocks
                .get(trx, block_id)
                .and_then(|b| b.to_ymap())
                .unwrap();

            // init default schema
            block.insert(trx, sys::FLAVOR, flavor.as_ref())?;
            block.insert(trx, sys::VERSION, ArrayPrelim::from([1, 0]))?;
            block.insert(
                trx,
                sys::CHILDREN,
                ArrayPrelim::<Vec<String>, String>::from(vec![]),
            )?;
            block.insert(
                trx,
                sys::CREATED,
                chrono::Utc::now().timestamp_millis() as f64,
            )?;

            space
                .updated
                .insert(trx, block_id, ArrayPrelim::<_, Any>::from([]))?;

            let children = block
                .get(trx, sys::CHILDREN)
                .and_then(|c| c.to_yarray())
                .unwrap();
            let updated = space.updated.get(trx, block_id).and_then(|c| c.to_yarray());

            let block = Self {
                id: space.id(),
                space_id: space.space_id(),
                doc: space.doc(),
                block_id: block_id.to_string(),
                operator,
                block,
                children,
                updated,
            };

            block.log_update(trx, HistoryOperation::Add);

            Ok(block)
        }
    }

    pub fn from<T, B>(trx: &T, space: &Space, block_id: B, operator: u64) -> Option<Block>
    where
        T: ReadTxn,
        B: AsRef<str>,
    {
        let block = space.blocks.get(trx, block_id.as_ref())?.to_ymap()?;
        let updated = space
            .updated
            .get(trx, block_id.as_ref())
            .and_then(|a| a.to_yarray());
        let children = block.get(trx, sys::CHILDREN)?.to_yarray()?;

        Some(Self {
            id: space.id(),
            space_id: space.space_id(),
            block_id: block_id.as_ref().to_string(),
            doc: space.doc(),
            operator,
            block,
            children,
            updated,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_parts<T: ReadTxn>(
        trx: &T,
        id: String,
        space_id: String,
        block_id: String,
        doc: &Doc,
        block: MapRef,
        updated: Option<ArrayRef>,
        operator: u64,
    ) -> Block {
        let children = block.get(trx, sys::CHILDREN).unwrap().to_yarray().unwrap();
        Self {
            id,
            space_id,
            block_id,
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

        self.updated.as_ref().map(|a| a.push_back(trx, array));
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

    pub fn set<T>(&self, trx: &mut TransactionMut, key: &str, value: T) -> JwstResult<()>
    where
        T: Into<Any>,
    {
        let key = format!("prop:{key}");
        match value.into() {
            Any::Bool(bool) => {
                self.block.insert(trx, key, bool)?;
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::String(text) => {
                self.block.insert(trx, key, text.to_string())?;
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::Number(number) => {
                self.block.insert(trx, key, number)?;
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.block.insert(trx, key, number as f64)?;
                } else {
                    self.block.insert(trx, key, number)?;
                }
                self.log_update(trx, HistoryOperation::Update);
            }
            Any::Null | Any::Undefined => {
                self.block.remove(trx, &key);
                self.log_update(trx, HistoryOperation::Delete);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }
        Ok(())
    }

    pub fn block_id(&self) -> String {
        self.block_id.clone()
    }

    // start with a namespace
    // for example: affine:text
    pub fn flavor<T>(&self, trx: &T) -> String
    where
        T: ReadTxn,
    {
        self.block
            .get(trx, sys::FLAVOR)
            .or_else(|| self.block.get(trx, sys::FLAVOUR))
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
            .as_ref()
            .and_then(|a| {
                a.iter(trx)
                    .filter_map(|v| v.to_yarray())
                    .last()
                    .and_then(|a| {
                        a.get(trx, 1).and_then(|i| match i.to_json(trx) {
                            Any::Number(n) => Some(n as u64),
                            _ => None,
                        })
                    })
            })
            .unwrap_or_else(|| self.created(trx))
    }

    pub fn history<T>(&self, trx: &T) -> Vec<BlockHistory>
    where
        T: ReadTxn,
    {
        self.updated
            .as_ref()
            .map(|a| {
                a.iter(trx)
                    .filter_map(|v| v.to_yarray())
                    .map(|v| (trx, v, self.block_id.clone()).into())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
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
    pub fn children_iter<T>(
        &self,
        cb: impl FnOnce(Box<dyn Iterator<Item = String> + '_>) -> T,
    ) -> T {
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

    fn set_parent(&self, trx: &mut TransactionMut, block_id: String) -> JwstResult<()> {
        self.block.insert(trx, sys::PARENT, block_id)?;
        Ok(())
    }

    pub fn push_children(&self, trx: &mut TransactionMut, block: &Block) -> JwstResult<()> {
        self.remove_children(trx, block)?;
        block.set_parent(trx, self.block_id.clone())?;

        self.children.push_back(trx, block.block_id.clone())?;

        self.log_update(trx, HistoryOperation::Add);

        Ok(())
    }

    pub fn insert_children_at(
        &self,
        trx: &mut TransactionMut,
        block: &Block,
        pos: u32,
    ) -> JwstResult<()> {
        self.remove_children(trx, block)?;
        block.set_parent(trx, self.block_id.clone())?;

        let children = &self.children;

        if children.len(trx) > pos {
            children.insert(trx, pos, block.block_id.clone())?;
        } else {
            children.push_back(trx, block.block_id.clone())?;
        }

        self.log_update(trx, HistoryOperation::Add);

        Ok(())
    }

    pub fn insert_children_before(
        &self,
        trx: &mut TransactionMut,
        block: &Block,
        reference: &str,
    ) -> JwstResult<()> {
        self.remove_children(trx, block)?;
        block.set_parent(trx, self.block_id.clone())?;

        let children = &self.children;

        if let Some(pos) = children
            .iter(trx)
            .position(|c| c.to_string(trx) == reference)
        {
            children.insert(trx, pos as u32, block.block_id.clone())?;
        } else {
            children.push_back(trx, block.block_id.clone())?;
        }

        self.log_update(trx, HistoryOperation::Add);

        Ok(())
    }

    pub fn insert_children_after(
        &self,
        trx: &mut TransactionMut,
        block: &Block,
        reference: &str,
    ) -> JwstResult<()> {
        self.remove_children(trx, block)?;
        block.set_parent(trx, self.block_id.clone())?;

        let children = &self.children;

        match children
            .iter(trx)
            .position(|c| c.to_string(trx) == reference)
        {
            Some(pos) if (pos as u32) < children.len(trx) => {
                children.insert(trx, pos as u32 + 1, block.block_id.clone())?;
            }
            _ => {
                children.push_back(trx, block.block_id.clone())?;
            }
        }

        self.log_update(trx, HistoryOperation::Add);

        Ok(())
    }

    pub fn remove_children(&self, trx: &mut TransactionMut, block: &Block) -> JwstResult<()> {
        let children = &self.children;
        block.set_parent(trx, self.block_id.clone())?;

        if let Some(current_pos) = children
            .iter(trx)
            .position(|c| c.to_string(trx) == block.block_id)
        {
            children.remove(trx, current_pos as u32)?;
            self.log_update(trx, HistoryOperation::Delete);
        }

        Ok(())
    }

    pub fn exists_children<T>(&self, trx: &T, block_id: &str) -> Option<usize>
    where
        T: ReadTxn,
    {
        self.children
            .iter(trx)
            .position(|c| c.to_string(trx) == block_id)
    }

    pub fn to_markdown<T>(&self, trx: &T, state: &mut MarkdownState) -> Option<String>
    where
        T: ReadTxn,
    {
        match self.get(trx, "text").map(|t| t.to_string()) {
            Some(text) => match self.flavor(trx).as_str() {
                "affine:code" => {
                    state.numbered_count = 0;
                    match self.get(trx, "language").map(|v| v.to_string()).as_deref() {
                        Some(language) => Some(format!("``` {}\n{}\n```\n", language, text)),
                        None => Some(format!("```\n{}\n```\n", text)),
                    }
                }
                format @ "affine:paragraph" => {
                    state.numbered_count = 0;
                    match self.get(trx, "type").map(|v| v.to_string()).as_deref() {
                        Some(
                            head @ "h1" | head @ "h2" | head @ "h3" | head @ "h4" | head @ "h5",
                        ) => Some(format!(
                            "{} {}\n",
                            "#".repeat(head[1..].parse().unwrap()),
                            text
                        )),
                        Some("quote") => Some(format!("> {text}\n")),
                        Some("text") => Some(format!("{text}\n")),
                        r#type @ Some(_) | r#type @ None => {
                            if let Some(r#type) = r#type {
                                warn!("Unprocessed format: {format}, {}", r#type);
                            } else {
                                warn!("Unprocessed format: {format}");
                            }
                            Some(text)
                        }
                    }
                }
                format @ "affine:list" => {
                    match self.get(trx, "type").map(|v| v.to_string()).as_deref() {
                        Some("numbered") => {
                            state.numbered_count += 1;
                            Some(format!("{}. {text}\n", state.numbered_count))
                        }
                        Some("todo") => {
                            state.numbered_count += 1;
                            let clicked = self
                                .get(trx, "checked")
                                .map(|v| v.to_string() == "true")
                                .unwrap_or(false);
                            Some(format!("[{}] {text}\n", if clicked { "x" } else { " " }))
                        }
                        Some("bulleted") => {
                            state.numbered_count += 1;
                            Some(format!("- {text}\n"))
                        }
                        r#type @ Some("text") | r#type @ Some(_) | r#type @ None => {
                            state.numbered_count = 0;
                            if let Some(r#type) = r#type {
                                warn!("Unprocessed format: {format}, {}", r#type);
                            } else {
                                warn!("Unprocessed format: {format}");
                            }
                            Some(text)
                        }
                    }
                }
                format => {
                    state.numbered_count = 0;
                    warn!("Unprocessed format: {format}");
                    Some(text)
                }
            },
            None => match self.flavor(trx).as_str() {
                "affine:divider" => {
                    state.numbered_count = 0;
                    Some("---\n".into())
                }
                "affine:embed" => {
                    state.numbered_count = 0;
                    match self.get(trx, "type").map(|v| v.to_string()).as_deref() {
                        Some("image") => self
                            .get(trx, "sourceId")
                            .map(|v| format!("![](/api/workspace/{}/blob/{})\n", self.id, v)),
                        _ => None,
                    }
                }
                format => {
                    state.numbered_count = 0;
                    warn!("Unprocessed format: {format}");
                    None
                }
            },
        }
    }
}

#[derive(Default)]
pub struct MarkdownState {
    numbered_count: usize,
}

impl Serialize for Block {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let trx = self.doc.transact();
        let any = self.block.to_json(&trx);

        let mut buffer = String::new();
        any.to_json(&mut buffer);
        let any: Value = serde_json::from_str(&buffer).unwrap();

        let mut block = any.as_object().unwrap().clone();
        block.insert("sys:id".to_string(), Value::String(self.block_id.clone()));

        Value::Object(block).serialize(serializer)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn init_block() {
        let workspace = Workspace::new("workspace");

        // new block
        workspace.with_trx(|mut t| {
            let space = t.get_space("space");

            let block = space.create(&mut t.trx, "test", "affine:text").unwrap();

            assert_eq!(block.id, "workspace");
            assert_eq!(block.space_id, "space");
            assert_eq!(block.block_id(), "test");
            assert_eq!(block.flavor(&t.trx), "affine:text");
            assert_eq!(block.version(&t.trx), [1, 0]);
        });

        // get exist block
        workspace.with_trx(|mut t| {
            let space = t.get_space("space");

            let block = space.get(&t.trx, "test").unwrap();

            assert_eq!(block.flavor(&t.trx), "affine:text");
            assert_eq!(block.version(&t.trx), [1, 0]);
        });
    }

    #[test]
    fn set_value() {
        let workspace = Workspace::new("test");

        workspace.with_trx(|mut t| {
            let space = t.get_space("space");

            let block = space.create(&mut t.trx, "test", "affine:text").unwrap();

            // normal type set
            block.set(&mut t.trx, "bool", true).unwrap();
            block.set(&mut t.trx, "text", "hello world").unwrap();
            block
                .set(&mut t.trx, "text_owned", "hello world".to_owned())
                .unwrap();
            block.set(&mut t.trx, "num", 123_i64).unwrap();
            block
                .set(&mut t.trx, "bigint", 9007199254740992_i64)
                .unwrap();

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
            let space = t.get_space("space");

            let block = space.create(&mut t.trx, "a", "affine:text").unwrap();
            let b = space.create(&mut t.trx, "b", "affine:text").unwrap();
            let c = space.create(&mut t.trx, "c", "affine:text").unwrap();
            let d = space.create(&mut t.trx, "d", "affine:text").unwrap();
            let e = space.create(&mut t.trx, "e", "affine:text").unwrap();
            let f = space.create(&mut t.trx, "f", "affine:text").unwrap();

            block.push_children(&mut t.trx, &b).unwrap();
            block.insert_children_at(&mut t.trx, &c, 0).unwrap();
            block.insert_children_before(&mut t.trx, &d, "b").unwrap();
            block.insert_children_after(&mut t.trx, &e, "b").unwrap();
            block.insert_children_after(&mut t.trx, &f, "c").unwrap();

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

            block.remove_children(&mut t.trx, &d).unwrap();

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
            let space = t.get_space("space");

            let block = space.create(&mut t.trx, "a", "affine:text").unwrap();

            block.set(&mut t.trx, "test", 1).unwrap();

            assert!(block.created(&t.trx) <= block.updated(&t.trx))
        });
    }

    #[test]
    fn history() {
        use yrs::Doc;

        let doc = Doc::with_client_id(123);

        let workspace = Workspace::from_doc(doc, "test");

        let (block, b, history) = workspace.with_trx(|mut t| {
            let space = t.get_space("space");
            let block = space.create(&mut t.trx, "a", "affine:text").unwrap();
            let b = space.create(&mut t.trx, "b", "affine:text").unwrap();

            block.set(&mut t.trx, "test", 1).unwrap();

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
            block.push_children(&mut t.trx, &b).unwrap();

            assert_eq!(block.exists_children(&t.trx, "b"), Some(0));

            block.remove_children(&mut t.trx, &b).unwrap();

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
            unreachable!();
        }
    }
}
