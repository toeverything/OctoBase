use super::{generate_interface, JwstBlock, WorkspaceTransaction};
use lib0::any::Any;

pub struct Block {
    pub(crate) block: JwstBlock,

    pub(crate) jwst_workspace: Option<jwst_core::Workspace>,
    pub(crate) jwst_block: Option<jwst_core::Block>,
}

impl Block {
    #[generate_interface(constructor)]
    pub fn new(
        trx: &mut WorkspaceTransaction,
        block_id: String,
        flavour: String,
        operator: u64,
    ) -> Block {
        let space = trx.trx.get_blocks();
        Self {
            block: JwstBlock::new(&mut trx.trx.trx, &space, &block_id, flavour, operator)
                .expect("failed to create block"),
            jwst_workspace: trx.jwst_ws.clone(),
            jwst_block: {
                let mut ws = trx.jwst_ws.clone();
                ws.as_mut()
                    .and_then(|ws| ws.get_blocks().ok())
                    .and_then(|b| b.get(block_id))
            },
        }
    }

    #[generate_interface]
    pub fn set_bool(&self, trx: &mut WorkspaceTransaction, key: String, value: bool) {
        self.block
            .set(&mut trx.trx.trx, &key, value)
            .expect("failed to set bool");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            if let Err(e) = block.set(&key, value) {
                println!("failed to set bool: {}", e);
            }
        }
    }

    #[generate_interface]
    pub fn set_string(&self, trx: &mut WorkspaceTransaction, key: String, value: String) {
        self.block
            .set(&mut trx.trx.trx, &key, value.clone())
            .expect("failed to set string");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            if let Err(e) = block.set(&key, value) {
                println!("failed to set string: {}", e);
            }
        }
    }

    #[generate_interface]
    pub fn set_float(&self, trx: &mut WorkspaceTransaction, key: String, value: f64) {
        self.block
            .set(&mut trx.trx.trx, &key, value)
            .expect("failed to set float");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            if let Err(e) = block.set(&key, value) {
                println!("failed to set float: {}", e);
            }
        }
    }

    #[generate_interface]
    pub fn set_integer(&self, trx: &mut WorkspaceTransaction, key: String, value: i64) {
        self.block
            .set(&mut trx.trx.trx, &key, value)
            .expect("failed to set integer");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            if let Err(e) = block.set(&key, value) {
                println!("failed to set integer: {}", e);
            }
        }
    }

    #[generate_interface]
    pub fn set_null(&self, trx: &mut WorkspaceTransaction, key: String) {
        self.block
            .set(&mut trx.trx.trx, &key, Any::Null)
            .expect("failed to set null");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            if let Err(e) = block.set(&key, jwst_core::Any::Null) {
                println!("failed to set null: {}", e);
            }
        }
    }

    #[generate_interface]
    pub fn is_bool(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.block
            .get(&trx.trx.trx, &key)
            .map(|a| matches!(a, Any::Bool(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_string(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.block
            .get(&trx.trx.trx, &key)
            .map(|a| matches!(a, Any::String(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_float(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.block
            .get(&trx.trx.trx, &key)
            .map(|a| matches!(a, Any::Number(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn is_integer(&self, trx: &WorkspaceTransaction, key: String) -> bool {
        self.block
            .get(&trx.trx.trx, &key)
            .map(|a| matches!(a, Any::BigInt(_)))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn get_bool(&self, trx: &WorkspaceTransaction, key: String) -> Option<i64> {
        self.block.get(&trx.trx.trx, &key).and_then(|a| match a {
            Any::Bool(i) => Some(i.into()),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_string(&self, trx: &WorkspaceTransaction, key: String) -> Option<String> {
        self.block.get(&trx.trx.trx, &key).and_then(|a| match a {
            Any::String(i) => Some(i.into()),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_float(&self, trx: &WorkspaceTransaction, key: String) -> Option<f64> {
        self.block.get(&trx.trx.trx, &key).and_then(|a| match a {
            Any::Number(i) => Some(i),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn get_integer(&self, trx: &WorkspaceTransaction, key: String) -> Option<i64> {
        self.block.get(&trx.trx.trx, &key).and_then(|a| match a {
            Any::BigInt(i) => Some(i),
            _ => None,
        })
    }

    #[generate_interface]
    pub fn id(&self) -> String {
        self.block.block_id()
    }

    #[generate_interface]
    pub fn flavour(&self, trx: &WorkspaceTransaction) -> String {
        self.block.flavour(&trx.trx.trx)
    }

    #[generate_interface]
    pub fn created(&self, trx: &WorkspaceTransaction) -> u64 {
        self.block.created(&trx.trx.trx)
    }

    #[generate_interface]
    pub fn updated(&self, trx: &WorkspaceTransaction) -> u64 {
        self.block.updated(&trx.trx.trx)
    }

    #[generate_interface]
    pub fn parent(&self, trx: &WorkspaceTransaction) -> Option<String> {
        self.block.parent(&trx.trx.trx)
    }

    #[generate_interface]
    pub fn children(&self, trx: &WorkspaceTransaction) -> Vec<String> {
        self.block.children(&trx.trx.trx)
    }

    #[generate_interface]
    pub fn push_children(&self, trx: &mut WorkspaceTransaction, block: &Block) {
        self.block
            .push_children(&mut trx.trx.trx, &block.block)
            .expect("failed to push children");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            let mut jwst_workspace = self.jwst_workspace.clone();
            if let Some(mut target_block) = jwst_workspace
                .as_mut()
                .and_then(|ws| ws.get_blocks().ok())
                .and_then(|s| s.get(block.block_id()))
            {
                if let Err(e) = block.push_children(&mut target_block) {
                    println!("failed to push children: {}", e);
                }
            }
        }
    }

    #[generate_interface]
    pub fn insert_children_at(&self, trx: &mut WorkspaceTransaction, block: &Block, pos: u32) {
        self.block
            .insert_children_at(&mut trx.trx.trx, &block.block, pos)
            .expect("failed to insert children at position");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            let mut jwst_workspace = self.jwst_workspace.clone();
            if let Some(mut target_block) = jwst_workspace
                .as_mut()
                .and_then(|ws| ws.get_blocks().ok())
                .and_then(|s| s.get(block.block_id()))
            {
                if let Err(e) = block.insert_children_at(&mut target_block, pos as u64) {
                    println!("failed to insert children at position: {}", e);
                }
            }
        }
    }

    #[generate_interface]
    pub fn insert_children_before(
        &self,
        trx: &mut WorkspaceTransaction,
        block: &Block,
        reference: &str,
    ) {
        self.block
            .insert_children_before(&mut trx.trx.trx, &block.block, reference)
            .expect("failed to insert children before");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            let mut jwst_workspace = self.jwst_workspace.clone();
            if let Some(mut target_block) = jwst_workspace
                .as_mut()
                .and_then(|ws| ws.get_blocks().ok())
                .and_then(|s| s.get(block.block_id()))
            {
                if let Err(e) = block.insert_children_before(&mut target_block, reference) {
                    println!("failed to insert children before: {}", e);
                }
            }
        }
    }

    #[generate_interface]
    pub fn insert_children_after(
        &self,
        trx: &mut WorkspaceTransaction,
        block: &Block,
        reference: &str,
    ) {
        self.block
            .insert_children_after(&mut trx.trx.trx, &block.block, reference)
            .expect("failed to insert children after");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            let mut jwst_workspace = self.jwst_workspace.clone();
            if let Some(mut target_block) = jwst_workspace
                .as_mut()
                .and_then(|ws| ws.get_blocks().ok())
                .and_then(|s| s.get(block.block_id()))
            {
                if let Err(e) = block.insert_children_after(&mut target_block, reference) {
                    println!("failed to insert children after: {}", e);
                }
            }
        }
    }

    #[generate_interface]
    pub fn remove_children(&self, trx: &mut WorkspaceTransaction, block: &Block) {
        self.block
            .remove_children(&mut trx.trx.trx, &block.block)
            .expect("failed to remove children");

        let mut block = self.jwst_block.clone();
        if let Some(block) = block.as_mut() {
            let mut jwst_workspace = self.jwst_workspace.clone();
            if let Some(mut target_block) = jwst_workspace
                .as_mut()
                .and_then(|ws| ws.get_blocks().ok())
                .and_then(|s| s.get(block.block_id()))
            {
                if let Err(e) = block.remove_children(&mut target_block) {
                    println!("failed to remove children: {}", e);
                }
            }
        }
    }

    #[generate_interface]
    pub fn exists_children(&self, trx: &WorkspaceTransaction, block_id: &str) -> i32 {
        self.block
            .exists_children(&trx.trx.trx, block_id)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }
}
