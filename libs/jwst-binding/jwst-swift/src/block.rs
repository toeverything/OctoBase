use super::DynamicValue;
use jwst::{Block as JwstBlock, Workspace};

pub struct Block {
    workspace: Workspace,
    block: JwstBlock,
}

impl Block {
    pub fn new(workspace: Workspace, block: JwstBlock) -> Self {
        Self { workspace, block }
    }

    pub fn get(&self, key: String) -> Option<DynamicValue> {
        self.workspace
            .with_trx(|trx| self.block.get(&trx.trx, &key).map(DynamicValue::new))
    }
}
