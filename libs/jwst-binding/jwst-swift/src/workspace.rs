use super::Block;
use jwst::Workspace as JwstWorkspace;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) jwst_workspace: Option<jwst_core::Workspace>,
    pub(crate) runtime: Arc<Runtime>,
}

impl Workspace {
    pub fn new(id: String, runtime: Arc<Runtime>) -> Self {
        Self {
            workspace: JwstWorkspace::new(&id),
            jwst_workspace: jwst_core::Workspace::new(id).ok(),
            runtime,
        }
    }

    pub fn id(&self) -> String {
        self.workspace.id()
    }

    pub fn client_id(&self) -> u64 {
        self.workspace.client_id()
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let runtime = self.runtime.clone();
            self.runtime
                .spawn(async move {
                    workspace.with_trx(|mut trx| {
                        let block = trx
                            .get_blocks()
                            .get(&trx.trx, &block_id)
                            .map(|b| Block::new(workspace.clone(), b, runtime));
                        drop(trx);
                        block
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn create(&self, block_id: String, flavour: String) -> Block {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let runtime = self.runtime.clone();
            self.runtime
                .spawn(async move {
                    workspace.with_trx(|mut trx| {
                        let block = Block::new(
                            workspace.clone(),
                            trx.get_blocks()
                                .create(&mut trx.trx, block_id, flavour)
                                .expect("failed to create block"),
                            runtime,
                        );
                        drop(trx);
                        block
                    })
                })
                .await
                .unwrap()
        })
    }

    pub fn search(self: &Workspace, query: String) -> String {
        self.workspace.search_result(query)
    }

    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        self.runtime.block_on(async {
            let workspace = self.workspace.clone();
            let runtime = self.runtime.clone();
            let flavour = flavour.to_string();
            self.runtime
                .spawn(async move {
                    workspace
                        .with_trx(|mut trx| {
                            trx.get_blocks().get_blocks_by_flavour(&trx.trx, &flavour)
                        })
                        .iter()
                        .map(|block| Block::new(workspace.clone(), block.clone(), runtime.clone()))
                        .collect::<Vec<_>>()
                })
                .await
                .unwrap()
        })
    }

    pub fn get_search_index(self: &Workspace) -> Vec<String> {
        self.workspace.metadata().search_index
    }

    pub fn set_search_index(self: &Workspace, fields: Vec<String>) -> bool {
        self.workspace
            .set_search_index(fields)
            .expect("failed to set search index")
    }
}
