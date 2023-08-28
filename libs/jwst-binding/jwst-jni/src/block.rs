use jwst_codec::Any;

use super::{generate_interface, JwstBlock};
use crate::workspace::Workspace;

pub struct Block {
    pub(crate) block: JwstBlock,
}

impl Block {
    #[generate_interface(constructor)]
    pub fn new(ws: &Workspace, block_id: String, flavour: String, operator: u64) -> Block {
        let mut space = ws
            .workspace
            .clone()
            .get_blocks()
            .expect("failed to get blocks from workspace");
        Self {
            block: JwstBlock::new(&mut space, &block_id, flavour, operator).expect("failed to create block"),
        }
    }

    pub(crate) fn from(block: JwstBlock) -> Self {
        Self { block }
    }

    #[generate_interface]
    pub fn set_bool(&self, key: String, value: bool) {
        self.block.clone().set(&key, value).expect("failed to set bool");
    }

    #[generate_interface]
    pub fn set_string(&self, key: String, value: String) {
        self.block
            .clone()
            .set(&key, value.clone())
            .expect("failed to set string");
    }

    #[generate_interface]
    pub fn set_float(&self, key: String, value: f64) {
        self.block.clone().set(&key, value).expect("failed to set float");
    }

    #[generate_interface]
    pub fn set_integer(&self, key: String, value: i64) {
        self.block.clone().set(&key, value).expect("failed to set integer");
    }

    #[generate_interface]
    pub fn set_null(&self, key: String) {
        self.block.clone().set(&key, Any::Null).expect("failed to set null");
    }

    #[generate_interface]
    pub fn is_bool(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::True | Any::False))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_string(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::String(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_float(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::Float32(_) | Any::Float64(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_integer(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::Integer(_) | Any::BigInt64(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn get_bool(&self, key: String) -> Option<i64> {
        self.block.get(&key).and_then(|a| match a {
            Any::True => Some(1),
            Any::False => Some(0),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_string(&self, key: String) -> Option<String> {
        self.block.get(&key).and_then(|a| match a {
            Any::String(i) => Some(i.into()),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_float(&self, key: String) -> Option<f64> {
        self.block.get(&key).and_then(|a| match a {
            Any::Float32(i) => Some(i.0 as f64),
            Any::Float64(i) => Some(i.0),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_integer(&self, key: String) -> Option<i64> {
        self.block.get(&key).and_then(|a| match a {
            Any::Integer(i) => Some(i as i64),
            Any::BigInt64(i) => Some(i),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn id(&self) -> String {
        self.block.block_id()
    }

    #[generate_interface]
    pub fn flavour(&self) -> String {
        self.block.flavour()
    }

    #[generate_interface]
    pub fn created(&self) -> u64 {
        self.block.created()
    }

    #[generate_interface]
    pub fn updated(&self) -> u64 {
        self.block.updated()
    }

    #[generate_interface]
    pub fn parent(&self) -> Option<String> {
        self.block.parent()
    }

    #[generate_interface]
    pub fn children(&self) -> Vec<String> {
        self.block.children()
    }

    #[generate_interface]
    pub fn push_children(&self, block: &Block) {
        self.block
            .clone()
            .push_children(&mut block.block.clone())
            .expect("failed to push children");
    }

    #[generate_interface]
    pub fn insert_children_at(&self, block: &Block, pos: u64) {
        self.block
            .clone()
            .insert_children_at(&mut block.block.clone(), pos)
            .expect("failed to insert children at position");
    }

    #[generate_interface]
    pub fn insert_children_before(&self, block: &Block, reference: &str) {
        self.block
            .clone()
            .insert_children_before(&mut block.block.clone(), reference)
            .expect("failed to insert children before");
    }

    #[generate_interface]
    pub fn insert_children_after(&self, block: &Block, reference: &str) {
        self.block
            .clone()
            .insert_children_after(&mut block.block.clone(), reference)
            .expect("failed to insert children after");
    }

    #[generate_interface]
    pub fn remove_children(&self, block: &Block) {
        self.block
            .clone()
            .remove_children(&mut block.block.clone())
            .expect("failed to remove children");
    }

    #[generate_interface]
    pub fn exists_children(&self, block_id: &str) -> i32 {
        self.block.exists_children(block_id).map(|i| i as i32).unwrap_or(-1)
    }
}
