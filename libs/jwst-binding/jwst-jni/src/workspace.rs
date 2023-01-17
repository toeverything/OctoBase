use super::{
    generate_interface, Block, JwstStorage, JwstWorkspace, OnWorkspaceTransaction,
    WorkspaceTransaction,
};
use jwst::error;
use yrs::{Subscription, UpdateEvent};

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    sub: Option<Subscription<UpdateEvent>>,
}

impl Workspace {
    #[generate_interface(constructor)]
    pub fn new(id: String) -> Workspace {
        Self {
            workspace: JwstWorkspace::new(id),
            sub: None,
        }
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
    pub fn get(&self, block_id: String) -> Option<Block> {
        self.workspace.get(block_id).map(Block)
    }

    #[generate_interface]
    pub fn exists(&self, block_id: &str) -> bool {
        self.workspace.exists(block_id)
    }

    #[generate_interface]
    pub fn with_trx(&self, on_trx: Box<dyn OnWorkspaceTransaction>) {
        self.workspace
            .with_trx(|trx| on_trx.on_trx(WorkspaceTransaction(trx)))
    }

    #[generate_interface]
    pub fn with_storage(&mut self, storage: JwstStorage) {
        let storage = storage.clone();
        let id = self.id();
        storage.reload(id.clone(), self.workspace.doc());
        self.sub = Some(self.workspace.observe(move |_, e| {
            if let Err(e) = storage.write_update(id.clone(), &e.update) {
                error!("Failed to write update to storage: {}", e);
            }
        }));
    }
}
