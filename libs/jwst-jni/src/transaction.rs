use super::{Block, JwstWorkspaceTransaction};

pub trait OnWorkspaceTransaction {
    fn on_trx(&self, trx: WorkspaceTransaction);
}

pub struct WorkspaceTransaction<'a>(pub(crate) JwstWorkspaceTransaction<'a>);

impl WorkspaceTransaction<'_> {
    pub fn remove(&mut self, block_id: String) -> bool {
        self.0.remove(&block_id)
    }

    pub fn create(&mut self, block_id: String, flavor: String) -> Block {
        Block(self.0.create(block_id, flavor))
    }
}
