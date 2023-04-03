use super::{Block, JwstWorkspaceTransaction};

pub trait OnWorkspaceTransaction {
    fn on_trx(&self, trx: WorkspaceTransaction);
}

pub struct WorkspaceTransaction<'a>(pub(crate) JwstWorkspaceTransaction<'a>);

impl WorkspaceTransaction<'_> {
    pub fn remove(&mut self, block_id: String) -> bool {
        self.0.get_blocks().remove(&mut self.0.trx, block_id)
    }

    pub fn create(&mut self, block_id: String, flavour: String) -> Block {
        Block(
            self.0
                .get_blocks()
                .create(&mut self.0.trx, block_id, flavour)
                .expect("failed to create block"),
        )
    }

    pub fn commit(&mut self) {
        self.0.commit()
    }
}

impl Drop for WorkspaceTransaction<'_> {
    fn drop(&mut self) {
        self.commit()
    }
}
