use super::{
    generate_interface, Block, JwstWorkspace, OnWorkspaceTransaction, WorkspaceTransaction,
};
use yrs::UpdateSubscription;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) sub: Option<UpdateSubscription>,
}

impl Workspace {
    #[generate_interface(constructor)]
    pub fn new(_id: String) -> Workspace {
        unimplemented!("Workspace::new")
    }

    #[generate_interface]
    pub fn id(&self) -> String {
        self.workspace.id()
    }

    #[generate_interface]
    pub fn client_id(&self) -> u64 {
        self.workspace.client_id()
    }

    #[generate_interface]
    pub fn get(&self, trx: &WorkspaceTransaction, block_id: String) -> Option<Block> {
        self.workspace.get(&trx.0.trx, block_id).map(Block)
    }

    #[generate_interface]
    pub fn exists(&self, trx: &WorkspaceTransaction, block_id: &str) -> bool {
        self.workspace.exists(&trx.0.trx, block_id)
    }

    #[generate_interface]
    pub fn with_trx(&self, on_trx: Box<dyn OnWorkspaceTransaction>) -> bool {
        self.workspace
            .try_with_trx(|trx| on_trx.on_trx(WorkspaceTransaction(trx)))
            .is_some()
    }

    #[generate_interface]
    pub fn drop_trx(&self, trx: WorkspaceTransaction) {
        drop(trx)
    }
}
