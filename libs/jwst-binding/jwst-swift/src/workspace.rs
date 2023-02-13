use super::Block;
use jwst::Workspace as JwstWorkspace;

pub struct Workspace {
    workspace: JwstWorkspace,
}

impl Workspace {
    pub fn new(id: String) -> Self {
        Self {
            workspace: JwstWorkspace::new(id),
        }
    }

    pub fn id(&self) -> String {
        self.workspace.id()
    }

    pub fn client_id(&self) -> u64 {
        self.workspace.client_id()
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        self.workspace.get(&block_id).map(Block::new)
    }

    pub fn create(&self, block_id: String, flavor: String) -> Block {
        self.workspace
            .with_trx(|trx| Block::new(trx.create(block_id, flavor)))
    }
}
