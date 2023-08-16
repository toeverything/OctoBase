use super::{Block, JwstWorkspaceTransaction};

pub trait OnWorkspaceTransaction {
    fn on_trx(&self, trx: WorkspaceTransaction);
}

pub struct WorkspaceTransaction<'a> {
    pub(crate) trx: JwstWorkspaceTransaction<'a>,
    pub(crate) jwst_ws: &'a Option<jwst_core::Workspace>,
}

impl WorkspaceTransaction<'_> {
    pub fn remove(&mut self, block_id: String) -> bool {
        if self.trx.get_blocks().remove(&mut self.trx.trx, &block_id) {
            let mut ws = self.jwst_ws.clone();
            ws.as_mut()
                .and_then(|ws| ws.get_blocks().ok())
                .map(|mut s| s.remove(block_id))
                .unwrap_or(false)
        } else {
            false
        }
    }

    pub fn create(&mut self, block_id: String, flavour: String) -> Block {
        let block = self
            .trx
            .get_blocks()
            .create(&mut self.trx.trx, &block_id, &flavour)
            .expect("failed to create block");
        let created = block.created(&self.trx.trx);

        Block {
            block,
            jwst_workspace: self.jwst_ws.clone(),
            jwst_block: {
                let mut ws = self.jwst_ws.clone();
                ws.as_mut()
                    .and_then(|ws| ws.get_blocks().ok())
                    .and_then(|mut b| b.create_ffi(block_id, flavour, created).ok())
            },
        }
    }

    pub fn commit(&mut self) {
        self.trx.commit()
    }
}

impl Drop for WorkspaceTransaction<'_> {
    fn drop(&mut self) {
        self.commit()
    }
}
