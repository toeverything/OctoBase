mod convert;

use std::fmt;

use jwst_codec::{Any, Array, Doc, Map, Text, Value};
use serde::{Serialize, Serializer};

use super::{constants::sys, *};

#[derive(Clone)]
pub struct Block {
    id: String,
    space_id: String,
    block_id: String,
    doc: Doc,
    operator: u64,
    block: Map,
    children: Array,
    updated: Map,
}

unsafe impl Send for Block {}
unsafe impl Sync for Block {}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockStruct")
            .field("id", &self.id)
            .field("space_id", &self.space_id)
            .field("block_id", &self.block_id)
            .field("doc", &self.doc)
            .field("operator", &self.operator)
            .field("block", &self.block)
            .field("children", &self.children)
            .field("updated", &self.updated.get(&self.block_id))
            .finish()
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        if self.id != other.id
            || self.space_id != other.space_id
            || self.block_id != other.block_id
            || self.doc != other.doc
            || self.operator != other.operator
            || self.block != other.block
            || self.children != other.children
            || self.updated != other.updated
        {
            return false;
        }
        true
    }
}

impl Block {
    // Create a new block, skip create if block is already created.
    pub fn new<B, F>(space: &mut Space, block_id: B, flavour: F, operator: u64) -> JwstResult<Block>
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        let block_id = block_id.as_ref();

        if let Some(block) = Self::from(space, block_id, operator) {
            Ok(block)
        } else {
            // init base struct
            space.blocks.insert(block_id.into(), space.doc().create_map()?)?;
            let mut block = space.blocks.get(block_id).and_then(|b| b.to_map()).unwrap();

            // init default schema
            block.insert(sys::FLAVOUR.into(), flavour.as_ref())?;
            block.insert(sys::CHILDREN.into(), space.doc().create_array()?)?;
            block.insert(sys::CREATED.into(), chrono::Utc::now().timestamp_millis())?;

            let children = block.get(sys::CHILDREN).and_then(|c| c.to_array()).unwrap();

            let mut block = Self {
                id: space.id(),
                space_id: space.space_id(),
                doc: space.doc(),
                block_id: block_id.to_string(),
                operator,
                block,
                children,
                updated: space.updated.clone(),
            };

            block.log_update()?;

            Ok(block)
        }
    }

    // only for jwst verify
    pub fn new_ffi<B, F>(space: &mut Space, block_id: B, flavour: F, operator: u64, created: u64) -> JwstResult<Block>
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        let block_id = block_id.as_ref();

        if let Some(block) = Self::from(space, block_id, operator) {
            Ok(block)
        } else {
            // init base struct
            space.blocks.insert(block_id.into(), space.doc().create_map()?)?;
            let mut block = space.blocks.get(block_id).and_then(|b| b.to_map()).unwrap();

            // init default schema
            block.insert(sys::FLAVOUR.into(), flavour.as_ref())?;
            block.insert(sys::CHILDREN.into(), space.doc().create_array()?)?;
            block.insert(sys::CREATED.into(), created)?;

            let children = block.get(sys::CHILDREN).and_then(|c| c.to_array()).unwrap();

            let mut block = Self {
                id: space.id(),
                space_id: space.space_id(),
                doc: space.doc(),
                block_id: block_id.to_string(),
                operator,
                block,
                children,
                updated: space.updated.clone(),
            };

            block.log_update()?;

            Ok(block)
        }
    }

    pub fn from<B>(space: &Space, block_id: B, operator: u64) -> Option<Block>
    where
        B: AsRef<str>,
    {
        let block = space.blocks.get(block_id.as_ref())?.to_map()?;
        let children = block.get(sys::CHILDREN)?.to_array()?;

        Some(Self {
            id: space.id(),
            space_id: space.space_id(),
            block_id: block_id.as_ref().to_string(),
            doc: space.doc(),
            operator,
            block,
            children,
            updated: space.updated.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_parts(
        id: String,
        space_id: String,
        block_id: String,
        doc: &Doc,
        block: Map,
        updated: Map,
        operator: u64,
    ) -> Block {
        let children = block.get(sys::CHILDREN).unwrap().to_array().unwrap();
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

    pub(crate) fn log_update(&mut self) -> JwstResult {
        self.updated.insert(
            self.block_id.to_string(),
            Any::Float64((chrono::Utc::now().timestamp_millis() as f64).into()),
        )?;

        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<Any> {
        let key = format!("prop:{key}");
        self.block.get(&key).and_then(|v| {
            match v {
                Value::Any(any) => match any {
                    Any::Integer(_)
                    | Any::Float32(_)
                    | Any::Float64(_)
                    | Any::BigInt64(_)
                    | Any::True
                    | Any::False
                    | Any::String(_) => return Some(any),
                    _ => {}
                },
                Value::Text(v) => return Some(Any::String(v.to_string())),
                _ => {}
            }
            error!("get wrong value at key {}", key);
            None
        })
    }

    pub fn set<T>(&mut self, key: &str, value: T) -> JwstResult
    where
        T: Into<Any>,
    {
        let key = format!("prop:{key}");
        match value.into() {
            Any::Null | Any::Undefined => {
                self.block.remove(&key);
                self.log_update()?;
            }
            value => {
                self.block.insert(key, value)?;
                self.log_update()?;
            }
        }
        Ok(())
    }

    pub fn block_id(&self) -> String {
        self.block_id.clone()
    }

    // start with a namespace
    // for example: affine:text
    pub fn flavour(&self) -> String {
        self.block.get(sys::FLAVOUR).map(|v| v.to_string()).unwrap_or_default()
    }

    pub fn created(&self) -> u64 {
        self.block
            .get(sys::CREATED)
            .and_then(|c| match c {
                Value::Any(Any::Integer(n)) => Some(n as u64),
                Value::Any(Any::BigInt64(n)) => Some(n as u64),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn updated(&self) -> u64 {
        self.updated
            .get(&self.block_id)
            .and_then(|i| match i {
                Value::Any(Any::BigInt64(n)) => Some(n as u64),
                _ => None,
            })
            .unwrap_or_else(|| self.created())
    }

    pub fn parent(&self) -> Option<String> {
        self.block.get(sys::PARENT).and_then(|c| {
            let s = c.to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
    }

    pub fn children(&self) -> Vec<String> {
        self.children.iter().map(|v| v.to_string()).collect()
    }

    #[inline]
    pub fn children_iter<T>(&self, cb: impl FnOnce(Box<dyn Iterator<Item = String> + '_>) -> T) -> T {
        let iterator = self.children.iter().map(|v| v.to_string());

        cb(Box::new(iterator))
    }

    pub fn children_len(&self) -> u64 {
        self.children.len()
    }

    pub fn children_exists<S>(&self, block_id: S) -> bool
    where
        S: AsRef<str>,
    {
        self.children
            .iter()
            .map(|v| v.to_string())
            .any(|bid| bid == block_id.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn content(&self) -> std::collections::HashMap<String, Any> {
        self.block
            .iter()
            .filter_map(|(key, val)| {
                val.to_any()
                    .and_then(|any| key.strip_prefix("prop:").map(|stripped| (stripped.to_owned(), any)))
            })
            .collect()
    }

    fn set_parent(&mut self, block_id: String) -> JwstResult {
        self.block.insert(sys::PARENT.into(), block_id)?;
        Ok(())
    }

    pub fn push_children(&mut self, block: &mut Block) -> JwstResult {
        self.remove_children(block)?;
        block.set_parent(self.block_id.clone())?;

        self.children.push(block.block_id.clone())?;

        self.log_update()?;

        Ok(())
    }

    pub fn insert_children_at(&mut self, block: &mut Block, pos: u64) -> JwstResult {
        self.remove_children(block)?;
        block.set_parent(self.block_id.clone())?;

        if self.children.len() > pos {
            self.children.insert(pos, block.block_id.clone())?;
        } else {
            self.children.push(block.block_id.clone())?;
        }

        self.log_update()?;

        Ok(())
    }

    pub fn insert_children_before(&mut self, block: &mut Block, reference: &str) -> JwstResult {
        self.remove_children(block)?;
        block.set_parent(self.block_id.clone())?;

        let pos = self.children.iter().position(|c| c.to_string() == reference);

        if let Some(pos) = pos {
            self.children.insert(pos as u64, block.block_id.clone())?;
        } else {
            self.children.push(block.block_id.clone())?;
        }

        self.log_update()?;

        Ok(())
    }

    pub fn insert_children_after(&mut self, block: &mut Block, reference: &str) -> JwstResult {
        self.remove_children(block)?;
        block.set_parent(self.block_id.clone())?;

        let pos = self.children.iter().position(|c| c.to_string() == reference);
        match pos {
            Some(pos) if (pos as u64) < self.children.len() => {
                self.children.insert(pos as u64 + 1, block.block_id.clone())?;
            }
            _ => {
                self.children.push(block.block_id.clone())?;
            }
        }

        self.log_update()?;

        Ok(())
    }

    pub fn remove_children(&mut self, block: &mut Block) -> JwstResult {
        block.set_parent(self.block_id.clone())?;

        let pos = self.children.iter().position(|c| c.to_string() == block.block_id);
        if let Some(current_pos) = pos {
            self.children.remove(current_pos as u64, 1)?;
            self.log_update()?;
        }

        Ok(())
    }

    pub fn exists_children(&self, block_id: &str) -> Option<usize> {
        self.children.iter().position(|c| c.to_string() == block_id)
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
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.block.len() as usize))?;
        for (key, value) in self.block.iter() {
            map.serialize_entry(&key, &value)?;
        }
        map.serialize_entry(constants::sys::ID, &self.block_id)?;

        map.end()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn init_block() {
        let mut workspace = Workspace::new("workspace").unwrap();

        // new block
        let mut space = workspace.get_space("space").unwrap();
        let block = space.create("test", "affine:text").unwrap();

        assert_eq!(block.id, "workspace");
        assert_eq!(block.space_id, "space");
        assert_eq!(block.block_id(), "test");
        assert_eq!(block.flavour(), "affine:text");

        // get exist block

        let space = workspace.get_space("space").unwrap();
        let block = space.get("test").unwrap();

        assert_eq!(block.flavour(), "affine:text");
    }

    #[test]
    fn set_value() {
        let mut workspace = Workspace::new("test").unwrap();

        let mut space = workspace.get_space("space").unwrap();
        let mut block = space.create("test", "affine:text").unwrap();

        // normal type set
        block.set("bool", true).unwrap();
        block.set("text", "hello world").unwrap();
        block.set("text_owned", "hello world".to_owned()).unwrap();
        block.set("num", 123_i32).unwrap();
        block.set("bigint", 9007199254740992_i64).unwrap();

        assert_eq!(block.get("bool").unwrap().to_string(), "true");
        assert_eq!(block.get("text").unwrap().to_string(), "hello world");
        assert_eq!(block.get("text_owned").unwrap().to_string(), "hello world");
        assert_eq!(block.get("num").unwrap().to_string(), "123");
        assert_eq!(block.get("bigint").unwrap().to_string(), "9007199254740992");

        assert_eq!(
            {
                let mut vec = block.content().into_iter().collect::<Vec<_>>();
                vec.sort_by(|a, b| a.0.cmp(&b.0));
                vec
            },
            vec![
                ("bigint".to_owned(), Any::BigInt64(9007199254740992)),
                ("bool".to_owned(), Any::True),
                ("num".to_owned(), Any::Integer(123)),
                ("text".to_owned(), Any::String("hello world".into())),
                ("text_owned".to_owned(), Any::String("hello world".into())),
            ]
        );
    }

    #[test]
    fn block_renew_value() {
        let mut workspace = Workspace::new("test").unwrap();

        let mut space = workspace.get_space("space").unwrap();
        let mut block = space.create("test", "affine:text").unwrap();

        let key = "hello".to_string();
        block.set(&key, "world").unwrap();

        let mut block = space.get("test").unwrap();
        block.set(&key, "12345678").unwrap();

        assert_eq!(block.get(&key).unwrap().to_string(), "12345678");
    }

    #[test]
    fn test_insert_remove_children() {
        let mut workspace = Workspace::new("text").unwrap();
        let mut space = workspace.get_space("space").unwrap();

        let mut block = space.create("a", "affine:text").unwrap();
        let mut b = space.create("b", "affine:text").unwrap();
        let mut c = space.create("c", "affine:text").unwrap();
        let mut d = space.create("d", "affine:text").unwrap();
        let mut e = space.create("e", "affine:text").unwrap();
        let mut f = space.create("f", "affine:text").unwrap();

        block.push_children(&mut b).unwrap();
        block.insert_children_at(&mut c, 0).unwrap();
        block.insert_children_before(&mut d, "b").unwrap();
        block.insert_children_after(&mut e, "b").unwrap();
        block.insert_children_after(&mut f, "c").unwrap();

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

        block.remove_children(&mut d).unwrap();

        assert_eq!(
            block.children(),
            vec!["c".to_owned(), "f".to_owned(), "b".to_owned(), "e".to_owned()]
        );
    }

    #[test]
    fn updated() {
        let mut workspace = Workspace::new("test").unwrap();
        let mut space = workspace.get_space("space").unwrap();

        let mut block = space.create("a", "affine:text").unwrap();

        block.set("test", 1).unwrap();

        assert!(block.created() <= block.updated())
    }
}
