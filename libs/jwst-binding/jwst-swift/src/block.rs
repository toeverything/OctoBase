use super::DynamicValue;
use jwst::{Block as JwstBlock, Workspace};
use lib0::any::Any;

pub struct Block {
    pub workspace: Workspace,
    pub block: JwstBlock,
}

impl Block {
    pub fn new(workspace: Workspace, block: JwstBlock) -> Self {
        Self { workspace, block }
    }

    pub fn get(&self, key: String) -> Option<DynamicValue> {
        self.workspace
            .with_trx(|trx| self.block.get(&trx.trx, &key).map(DynamicValue::new))
    }

    pub fn children(&self) -> Vec<String> {
        self.workspace.with_trx(|trx| self.block.children(&trx.trx))
    }

    pub fn push_children(&self, block: &Block) {
        self.workspace
            .with_trx(|mut trx| self.block.push_children(&mut trx.trx, &block.block))
            .expect("failed to push children");
    }

    pub fn insert_children_at(&self, block: &Block, pos: u32) {
        self.workspace
            .with_trx(|mut trx| {
                self.block
                    .insert_children_at(&mut trx.trx, &block.block, pos)
            })
            .expect("failed to insert children at position");
    }

    pub fn insert_children_before(&self, block: &Block, reference: &str) {
        self.workspace
            .with_trx(|mut trx| {
                self.block
                    .insert_children_before(&mut trx.trx, &block.block, reference)
            })
            .expect("failed to insert children before");
    }

    pub fn insert_children_after(&self, block: &Block, reference: &str) {
        self.workspace
            .with_trx(|mut trx| {
                self.block
                    .insert_children_after(&mut trx.trx, &block.block, reference)
            })
            .expect("failed to insert children after");
    }

    pub fn remove_children(&self, block: &Block) {
        self.workspace
            .with_trx(|mut trx| self.block.remove_children(&mut trx.trx, &block.block))
            .expect("failed to remove children");
    }

    pub fn exists_children(&self, block_id: &str) -> i32 {
        self.workspace
            .with_trx(|trx| self.block.exists_children(&trx.trx, block_id))
            .map(|i| i as i32)
            .unwrap_or(-1)
    }

    pub fn parent(&self) -> String {
        self.workspace
            .with_trx(|trx| self.block.parent(&trx.trx).unwrap())
    }

    pub fn updated(&self) -> u64 {
        self.workspace.with_trx(|trx| self.block.updated(&trx.trx))
    }

    pub fn id(&self) -> String {
        self.block.block_id()
    }

    pub fn flavor(&self) -> String {
        self.workspace.with_trx(|trx| self.block.flavor(&trx.trx))
    }

    pub fn version(&self) -> String {
        self.workspace.with_trx(|trx| {
            let [major, minor] = self.block.version(&trx.trx);
            format!("{major}.{minor}")
        })
    }

    pub fn created(&self) -> u64 {
        self.workspace.with_trx(|trx| self.block.created(&trx.trx))
    }

    pub fn set_bool(&self, key: String, value: bool) {
        self.workspace
            .with_trx(|mut trx| self.block.set(&mut trx.trx, &key, value))
            .expect("failed to set bool");
    }

    pub fn set_string(&self, key: String, value: String) {
        self.workspace
            .with_trx(|mut trx| self.block.set(&mut trx.trx, &key, value))
            .expect("failed to set string");
    }

    pub fn set_float(&self, key: String, value: f64) {
        self.workspace
            .with_trx(|mut trx| self.block.set(&mut trx.trx, &key, value))
            .expect("failed to set float");
    }

    pub fn set_integer(&self, key: String, value: i64) {
        self.workspace
            .with_trx(|mut trx| self.block.set(&mut trx.trx, &key, value))
            .expect("failed to set integer");
    }

    pub fn set_null(&self, key: String) {
        self.workspace
            .with_trx(|mut trx| self.block.set(&mut trx.trx, &key, Any::Null))
            .expect("failed to set null");
    }

    pub fn is_bool(&self, key: String) -> bool {
        self.workspace.with_trx(|trx| {
            self.block
                .get(&trx.trx, &key)
                .map(|a| matches!(a, Any::Bool(_)))
                .unwrap_or(false)
        })
    }

    pub fn is_string(&self, key: String) -> bool {
        self.workspace.with_trx(|trx| {
            self.block
                .get(&trx.trx, &key)
                .map(|a| matches!(a, Any::String(_)))
                .unwrap_or(false)
        })
    }

    pub fn is_float(&self, key: String) -> bool {
        self.workspace.with_trx(|trx| {
            self.block
                .get(&trx.trx, &key)
                .map(|a| matches!(a, Any::Number(_)))
                .unwrap_or(false)
        })
    }

    pub fn is_integer(&self, key: String) -> bool {
        self.workspace.with_trx(|trx| {
            self.block
                .get(&trx.trx, &key)
                .map(|a| matches!(a, Any::BigInt(_)))
                .unwrap_or(false)
        })
    }

    pub fn get_bool(&self, key: String) -> Option<i64> {
        self.workspace.with_trx(|trx| {
            self.block.get(&trx.trx, &key).and_then(|a| match a {
                Any::Bool(i) => Some(i.into()),
                _ => None,
            })
        })
    }

    pub fn get_string(&self, key: String) -> Option<String> {
        self.workspace.with_trx(|trx| {
            self.block.get(&trx.trx, &key).and_then(|a| match a {
                Any::String(i) => Some(i.into()),
                _ => None,
            })
        })
    }

    pub fn get_float(&self, key: String) -> Option<f64> {
        self.workspace.with_trx(|trx| {
            self.block.get(&trx.trx, &key).and_then(|a| match a {
                Any::Number(i) => Some(i),
                _ => None,
            })
        })
    }

    pub fn get_integer(&self, key: String) -> Option<i64> {
        self.workspace.with_trx(|trx| {
            self.block.get(&trx.trx, &key).and_then(|a| match a {
                Any::BigInt(i) => Some(i),
                _ => None,
            })
        })
    }
}
