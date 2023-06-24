use super::{
    generate_interface, Block, JwstWorkspace, OnWorkspaceTransaction, VecOfStrings,
    WorkspaceTransaction,
};
use crate::block_observer::{BlockObserver, BlockObserverWrapper};
use std::sync::Arc;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
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
    pub fn get(&self, trx: &mut WorkspaceTransaction, block_id: String) -> Option<Block> {
        trx.0.get_blocks().get(&trx.0.trx, block_id).map(Block)
    }

    #[generate_interface]
    pub fn exists(&self, trx: &mut WorkspaceTransaction, block_id: &str) -> bool {
        trx.0.get_blocks().exists(&trx.0.trx, block_id)
    }

    #[generate_interface]
    pub fn with_trx(&self, on_trx: Box<dyn OnWorkspaceTransaction>) -> bool {
        self.workspace
            .try_with_trx(|trx| on_trx.on_trx(WorkspaceTransaction(trx)))
            .is_some()
    }

    #[generate_interface]
    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        self.workspace.with_trx(|mut trx| {
            trx.get_blocks()
                .get_blocks_by_flavour(&trx.trx, flavour)
                .iter()
                .map(|item| Block(item.clone()))
                .collect()
        })
    }

    #[generate_interface]
    pub fn drop_trx(&self, trx: WorkspaceTransaction) {
        drop(trx)
    }

    #[generate_interface]
    pub fn search(&self, query: String) -> String {
        self.workspace.search_result(query)
    }

    #[generate_interface]
    pub fn get_search_index(&self) -> Vec<String> {
        self.workspace.metadata().search_index
    }

    #[generate_interface]
    pub fn set_search_index(&self, fields: VecOfStrings) -> bool {
        self.workspace
            .set_search_index(fields)
            .expect("failed to set search index")
    }

    #[generate_interface]
    pub fn set_callback(&self, observer: Box<dyn BlockObserver>) -> bool {
        let observer = BlockObserverWrapper::new(observer);
        self.workspace
            .set_callback(Arc::new(Box::new(move |_workspace_id, block_ids| {
                observer.on_change(block_ids);
            })));
        true
    }
}
