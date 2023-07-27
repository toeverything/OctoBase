use super::DynamicValue;
use jwst::{Block as JwstBlock, Workspace};
use lib0::any::Any;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct Block {
    pub workspace: Workspace,
    pub block: JwstBlock,
    runtime: Arc<Runtime>,
}

impl Block {
    pub fn new(workspace: Workspace, block: JwstBlock, runtime: Arc<Runtime>) -> Self {
        Self {
            workspace,
            block,
            runtime,
        }
    }

    pub fn get(&self, key: String) -> Option<DynamicValue> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| block.get(&trx.trx, &key).map(DynamicValue::new))
                })
                .await
                .unwrap()
        })
    }

    pub fn children(&self) -> Vec<String> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || workspace.with_trx(|trx| block.children(&trx.trx)))
                .await
                .unwrap()
        })
    }

    pub fn push_children(&self, block: &Block) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            let target_block = block.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| curr_block.push_children(&mut trx.trx, &target_block))
                        .expect("failed to push children")
                })
                .await
                .unwrap()
        })
    }

    pub fn insert_children_at(&self, block: &Block, pos: u32) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            let target_block = block.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| {
                            curr_block.insert_children_at(&mut trx.trx, &target_block, pos)
                        })
                        .expect("failed to insert children at position");
                })
                .await
                .unwrap()
        })
    }

    pub fn insert_children_before(&self, block: &Block, reference: &str) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            let target_block = block.block.clone();
            let reference = reference.to_string();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| {
                            curr_block.insert_children_before(
                                &mut trx.trx,
                                &target_block,
                                &reference,
                            )
                        })
                        .expect("failed to insert children before");
                })
                .await
                .unwrap()
        })
    }

    pub fn insert_children_after(&self, block: &Block, reference: &str) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            let target_block = block.block.clone();
            let reference = reference.to_string();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| {
                            curr_block.insert_children_after(
                                &mut trx.trx,
                                &target_block,
                                &reference,
                            )
                        })
                        .expect("failed to insert children after");
                })
                .await
                .unwrap()
        })
    }

    pub fn remove_children(&self, block: &Block) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            let target_block = block.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| curr_block.remove_children(&mut trx.trx, &target_block))
                        .expect("failed to remove children");
                })
                .await
                .unwrap()
        })
    }

    pub fn exists_children(&self, block_id: &str) -> i32 {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            let block_id = block_id.to_string();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|trx| curr_block.exists_children(&trx.trx, &block_id))
                        .map(|i| i as i32)
                        .unwrap_or(-1)
                })
                .await
                .unwrap()
        })
    }

    pub fn parent(&self) -> String {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let curr_block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| curr_block.parent(&trx.trx).unwrap())
                })
                .await
                .unwrap()
        })
    }

    pub fn updated(&self) -> u64 {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || workspace.with_trx(|trx| block.updated(&trx.trx)))
                .await
                .unwrap()
        })
    }

    pub fn id(&self) -> String {
        self.block.block_id()
    }

    pub fn flavour(&self) -> String {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || workspace.with_trx(|trx| block.flavour(&trx.trx)))
                .await
                .unwrap()
        })
    }

    pub fn created(&self) -> u64 {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || workspace.with_trx(|trx| block.created(&trx.trx)))
                .await
                .unwrap()
        })
    }

    pub fn set_bool(&self, key: String, value: bool) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| block.set(&mut trx.trx, &key, value))
                        .expect("failed to set bool");
                })
                .await
                .unwrap()
        })
    }

    pub fn set_string(&self, key: String, value: String) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| block.set(&mut trx.trx, &key, value))
                        .expect("failed to set string");
                })
                .await
                .unwrap()
        })
    }

    pub fn set_float(&self, key: String, value: f64) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| block.set(&mut trx.trx, &key, value))
                        .expect("failed to set float");
                })
                .await
                .unwrap()
        })
    }

    pub fn set_integer(&self, key: String, value: i64) {
        let workspace = self.workspace.clone();
        let block = self.block.clone();
        self.runtime.block_on(async {
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| block.set(&mut trx.trx, &key, value))
                        .expect("failed to set integer");
                })
                .await
                .unwrap()
        })
    }

    pub fn set_null(&self, key: String) {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace
                        .with_trx(|mut trx| block.set(&mut trx.trx, &key, Any::Null))
                        .expect("failed to set null");
                })
                .await
                .unwrap()
        })
    }

    pub fn is_bool(&self, key: String) -> bool {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block
                            .get(&trx.trx, &key)
                            .map(|a| matches!(a, Any::Bool(_)))
                            .unwrap_or(false)
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn is_string(&self, key: String) -> bool {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block
                            .get(&trx.trx, &key)
                            .map(|a| matches!(a, Any::String(_)))
                            .unwrap_or(false)
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn is_float(&self, key: String) -> bool {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block
                            .get(&trx.trx, &key)
                            .map(|a| matches!(a, Any::Number(_)))
                            .unwrap_or(false)
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn is_integer(&self, key: String) -> bool {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block
                            .get(&trx.trx, &key)
                            .map(|a| matches!(a, Any::BigInt(_)))
                            .unwrap_or(false)
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn get_bool(&self, key: String) -> Option<i64> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block.get(&trx.trx, &key).and_then(|a| match a {
                            Any::Bool(i) => Some(i.into()),
                            _ => None,
                        })
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn get_string(&self, key: String) -> Option<String> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block.get(&trx.trx, &key).and_then(|a| match a {
                            Any::String(i) => Some(i.into()),
                            _ => None,
                        })
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn get_float(&self, key: String) -> Option<f64> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block.get(&trx.trx, &key).and_then(|a| match a {
                            Any::Number(i) => Some(i),
                            _ => None,
                        })
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn get_integer(&self, key: String) -> Option<i64> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let block = self.block.clone();
            self.runtime
                .spawn_blocking(move || {
                    workspace.with_trx(|trx| {
                        block.get(&trx.trx, &key).and_then(|a| match a {
                            Any::BigInt(i) => Some(i),
                            _ => None,
                        })
                    })
                })
                .await
                .unwrap()
        })
    }
}
