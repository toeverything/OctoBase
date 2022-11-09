use super::{
    generate_interface, Block, JwstWorkspace, OnWorkspaceTransaction, WorkspaceTransaction,
};

pub struct Workspace(pub(crate) JwstWorkspace);

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

    #[generate_interface]
    pub fn with_trx(&self, on_trx: Box<dyn OnWorkspaceTransaction>) {
        self.0
            .with_trx(|trx| on_trx.on_trx(WorkspaceTransaction(trx)))
    }
}
