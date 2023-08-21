use jwst_core::{Any, Block as JwstBlock};

use super::*;

pub struct Block {
    pub(crate) block: JwstBlock,
}

impl Block {
    pub fn new(block: JwstBlock) -> Self {
        Self { block }
    }

    pub fn get(&self, key: String) -> Option<DynamicValue> {
        self.block.get(&key).map(DynamicValue::new)
    }

    pub fn children(&self) -> Vec<String> {
        self.block.children()
    }

    pub fn push_children(&self, target_block: &Block) {
        let mut block = self.block.clone();
        let mut target_block = target_block.block.clone();

        block.push_children(&mut target_block).expect("failed to push children");
    }

    pub fn insert_children_at(&self, target_block: &Block, pos: u32) {
        let mut block = self.block.clone();
        let mut target_block = target_block.block.clone();

        block
            .insert_children_at(&mut target_block, pos as u64)
            .expect("failed to insert children at position");
    }

    pub fn insert_children_before(&self, target_block: &Block, reference: &str) {
        let mut block = self.block.clone();
        let mut target_block = target_block.block.clone();

        block
            .insert_children_before(&mut target_block, &reference)
            .expect("failed to insert children before");
    }

    pub fn insert_children_after(&self, target_block: &Block, reference: &str) {
        let mut block = self.block.clone();
        let mut target_block = target_block.block.clone();

        block
            .insert_children_after(&mut target_block, &reference)
            .expect("failed to insert children after");
    }

    pub fn remove_children(&self, target_block: &Block) {
        let mut block = self.block.clone();
        let mut target_block = target_block.block.clone();

        block
            .remove_children(&mut target_block)
            .expect("failed to remove jwst block");
    }

    pub fn exists_children(&self, block_id: &str) -> i32 {
        self.block.exists_children(block_id).map(|i| i as i32).unwrap_or(-1)
    }

    pub fn parent(&self) -> String {
        self.block.parent().unwrap()
    }

    pub fn updated(&self) -> u64 {
        self.block.updated()
    }

    pub fn id(&self) -> String {
        self.block.block_id()
    }

    pub fn flavour(&self) -> String {
        self.block.flavour()
    }

    pub fn created(&self) -> u64 {
        self.block.created()
    }

    pub fn set_bool(&self, key: String, value: bool) {
        let mut block = self.block.clone();
        block
            .set(&key, value)
            .expect(&format!("failed to set bool: {} {}", key, value))
    }

    pub fn set_string(&self, key: String, value: String) {
        let mut block = self.block.clone();
        block
            .set(&key, value.clone())
            .expect(&format!("failed to set string: {} {}", key, value))
    }

    pub fn set_float(&self, key: String, value: f64) {
        let mut block = self.block.clone();
        block
            .set(&key, value)
            .expect(&format!("failed to set float: {} {}", key, value));
    }

    pub fn set_integer(&self, key: String, value: i64) {
        let mut block = self.block.clone();
        block
            .set(&key, value)
            .expect(&format!("failed to set integer: {} {}", key, value));
    }

    pub fn set_null(&self, key: String) {
        let mut block = self.block.clone();
        block
            .set(&key, Any::Null)
            .expect(&format!("failed to set null: {}", key));
    }

    pub fn is_bool(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::True | Any::False))
            .unwrap_or(false)
    }

    pub fn is_string(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::String(_)))
            .unwrap_or(false)
    }

    pub fn is_float(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::Float32(_) | Any::Float64(_)))
            .unwrap_or(false)
    }

    pub fn is_integer(&self, key: String) -> bool {
        self.block
            .get(&key)
            .map(|a| matches!(a, Any::Integer(_) | Any::BigInt64(_)))
            .unwrap_or(false)
    }

    pub fn get_bool(&self, key: String) -> Option<bool> {
        self.block.get(&key).and_then(|a| match a {
            Any::True => Some(true),
            Any::False => Some(false),
            _ => None,
        })
    }

    pub fn get_string(&self, key: String) -> Option<String> {
        self.block.get(&key).and_then(|a| match a {
            Any::String(s) => Some(s),
            _ => None,
        })
    }

    pub fn get_float(&self, key: String) -> Option<f64> {
        self.block.get(&key).and_then(|a| match a {
            Any::Float32(f) => Some(f.0 as f64),
            Any::Float64(f) => Some(f.0),
            _ => None,
        })
    }

    pub fn get_integer(&self, key: String) -> Option<i64> {
        self.block.get(&key).and_then(|a| match a {
            Any::Integer(i) => Some(i as i64),
            Any::BigInt64(i) => Some(i),
            _ => None,
        })
    }
}
