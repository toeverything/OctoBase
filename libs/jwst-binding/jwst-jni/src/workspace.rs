use super::{
    generate_interface, Block, JwstStorage, JwstWorkspace, OnWorkspaceTransaction,
    WorkspaceTransaction,
};
use jwst::{error, info};
use yrs::UpdateSubscription;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    sub: Option<UpdateSubscription>,
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

    #[generate_interface]
    pub fn with_storage(&mut self, storage: JwstStorage, remote: String) {
        let id = self.id();
        storage.reload(id.clone(), self.workspace.doc());
        info!("remote: {}", remote);
        if !remote.is_empty() {
            if let Err(e) = storage.connect(id.clone(), remote) {
                error!("Failed to connect to remote: {}", e);
            }
        }
        self.sub = self.workspace.observe(move |_, e| {
            if let Err(e) = storage.write_update(id.clone(), &e.update) {
                error!("Failed to write update to storage: {}", e);
            }
        });
    }
}
