use super::{generate_interface, JwstBlock, WorkspaceTransaction};
use lib0::any::Any;

pub struct Block(pub(crate) JwstBlock);

impl Block {
    #[generate_interface(constructor)]
    pub fn new(
        trx: &mut WorkspaceTransaction,
        block_id: String,
        flavour: String,
        operator: u64,
    ) -> Block {
        let space = trx.0.get_blocks();
        Self(
            JwstBlock::new(&mut trx.0.trx, &space, block_id, flavour, operator)
                .expect("failed to create block"),
        )
    }

    #[generate_interface]
    pub fn set_bool(&self, trx: &mut WorkspaceTransaction, key: String, value: bool) {
        self.0
            .set(&mut trx.0.trx, &key, value)
            .expect("failed to set bool");
    }

    #[generate_interface]
    pub fn set_string(&self, trx: &mut WorkspaceTransaction, key: String, value: String) {
        self.0
            .set(&mut trx.0.trx, &key, value)
            .expect("failed to set string");
    }

    #[generate_interface]
    pub fn set_float(&self, trx: &mut WorkspaceTransaction, key: String, value: f64) {
        self.0
            .set(&mut trx.0.trx, &key, value)
            .expect("failed to set float");
    }

    #[generate_interface]
    pub fn set_integer(&self, trx: &mut WorkspaceTransaction, key: String, value: i64) {
        self.0
            .set(&mut trx.0.trx, &key, value)
            .expect("failed to set integer");
    }

    #[generate_interface]
    pub fn set_null(&self, trx: &mut WorkspaceTransaction, key: String) {
        self.0
            .set(&mut trx.0.trx, &key, Any::Null)
            .expect("failed to set null");
    }

    #[generate_interface]
    pub fn is_bool(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.0
            .get(&trx.0.trx, &key)
            .map(|a| matches!(a, Any::Bool(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_string(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.0
            .get(&trx.0.trx, &key)
            .map(|a| matches!(a, Any::String(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_float(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.0
            .get(&trx.0.trx, &key)
            .map(|a| matches!(a, Any::Number(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_integer(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.0
            .get(&trx.0.trx, &key)
            .map(|a| matches!(a, Any::BigInt(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn get_bool(&self, trx: &WorkspaceTransaction, key: String) -> Option<i64> {
        self.0.get(&trx.0.trx, &key).and_then(|a| match a {
            Any::Bool(i) => Some(i.into()),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_string(&self, trx: &WorkspaceTransaction, key: String) -> Option<String> {
        self.0.get(&trx.0.trx, &key).and_then(|a| match a {
            Any::String(i) => Some(i.into()),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_float(&self, trx: &WorkspaceTransaction, key: String) -> Option<f64> {
        self.0.get(&trx.0.trx, &key).and_then(|a| match a {
            Any::Number(i) => Some(i),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_integer(&self, trx: &WorkspaceTransaction, key: String) -> Option<i64> {
        self.0.get(&trx.0.trx, &key).and_then(|a| match a {
            Any::BigInt(i) => Some(i),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn id(&self) -> String {
        self.0.block_id()
    }

    #[generate_interface]
    pub fn flavour(&self, trx: &WorkspaceTransaction) -> String {
        self.0.flavour(&trx.0.trx)
    }

    #[generate_interface]
    pub fn created(&self, trx: &WorkspaceTransaction) -> u64 {
        self.0.created(&trx.0.trx)
    }

    #[generate_interface]
    pub fn updated(&self, trx: &WorkspaceTransaction) -> u64 {
        self.0.updated(&trx.0.trx)
    }

    #[generate_interface]
    pub fn parent(&self, trx: &WorkspaceTransaction) -> Option<String> {
        self.0.parent(&trx.0.trx)
    }

    #[generate_interface]
    pub fn children(&self, trx: &WorkspaceTransaction) -> Vec<String> {
        self.0.children(&trx.0.trx)
    }

    #[generate_interface]
    pub fn push_children(&self, trx: &mut WorkspaceTransaction, block: &Block) {
        self.0
            .push_children(&mut trx.0.trx, &block.0)
            .expect("failed to push children")
    }

    #[generate_interface]
    pub fn insert_children_at(&self, trx: &mut WorkspaceTransaction, block: &Block, pos: u32) {
        self.0
            .insert_children_at(&mut trx.0.trx, &block.0, pos)
            .expect("failed to insert children at position")
    }

    #[generate_interface]
    pub fn insert_children_before(
        &self,
        trx: &mut WorkspaceTransaction,
        block: &Block,
        reference: &str,
    ) {
        self.0
            .insert_children_before(&mut trx.0.trx, &block.0, reference)
            .expect("failed to insert children before")
    }

    #[generate_interface]
    pub fn insert_children_after(
        &self,
        trx: &mut WorkspaceTransaction,
        block: &Block,
        reference: &str,
    ) {
        self.0
            .insert_children_after(&mut trx.0.trx, &block.0, reference)
            .expect("failed to insert children after")
    }

    #[generate_interface]
    pub fn remove_children(&self, trx: &mut WorkspaceTransaction, block: &Block) {
        self.0
            .remove_children(&mut trx.0.trx, &block.0)
            .expect("failed to remove children");
    }

    #[generate_interface]
    pub fn exists_children(&self, trx: &WorkspaceTransaction, block_id: &str) -> i32 {
        self.0
            .exists_children(&trx.0.trx, block_id)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }
}
