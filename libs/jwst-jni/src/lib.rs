mod java_glue;

pub use crate::java_glue::*;

use jwst::{
    Block as JwstBlock, Workspace as JwstWorkspace,
    WorkspaceTransaction as JwstWorkspaceTransaction,
};
use rifgen::rifgen_attr::*;

pub struct WorkspaceTransaction<'a>(JwstWorkspaceTransaction<'a>);

impl WorkspaceTransaction<'_> {
    pub fn remove(&mut self, block_id: String) -> bool {
        self.0.remove(&block_id)
    }

    pub fn create(&mut self, block_id: String, flavor: String) -> Block {
        Block(self.0.create(block_id, flavor))
    }
}

pub struct Workspace(JwstWorkspace);

impl Workspace {
    #[generate_interface(constructor)]
    pub fn new(id: String) -> Workspace {
        Self(JwstWorkspace::new(id))
    }

    #[generate_interface]
    pub fn id(&self) -> String {
        self.0.id()
    }

    #[generate_interface]
    pub fn client_id(&self) -> u64 {
        self.0.client_id()
    }

    #[generate_interface]
    pub fn get_trx(&self) -> WorkspaceTransaction {
        WorkspaceTransaction(self.0.get_trx())
    }

    #[generate_interface]
    pub fn get(&self, block_id: String) -> Option<Block> {
        self.0.get(block_id).map(Block)
    }

    #[generate_interface]
    pub fn exists(&self, block_id: &str) -> bool {
        self.0.exists(block_id)
    }
}

pub struct Block(JwstBlock);

impl Block {
    #[generate_interface(constructor)]
    pub fn new(workspace: &Workspace, block_id: String, flavor: String, operator: u64) -> Block {
        let mut trx = workspace.get_trx().0.trx;
        Self(JwstBlock::new(
            &workspace.0,
            &mut trx,
            block_id,
            flavor,
            operator,
        ))
    }
}
