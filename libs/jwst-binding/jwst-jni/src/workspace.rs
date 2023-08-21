use std::sync::Arc;

use jwst_rpc::workspace_compare;
use tokio::{runtime::Runtime, sync::mpsc::Sender};

use super::*;
use crate::block_observer::{BlockObserver, BlockObserverWrapper};

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) jwst_workspace: Option<jwst_core::Workspace>,
    #[allow(dead_code)]
    pub(crate) runtime: Arc<Runtime>,

    pub(crate) sender: Sender<Log>,
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
    pub fn get(&mut self, trx: &mut WorkspaceTransaction, block_id: String) -> Option<Block> {
        trx.trx.get_blocks().get(&trx.trx.trx, block_id).map(|block| Block {
            jwst_workspace: self.jwst_workspace.clone(),
            jwst_block: {
                let mut ws = self.jwst_workspace.clone();
                ws.as_mut()
                    .and_then(|ws| ws.get_blocks().ok())
                    .and_then(|s| s.get(block.block_id()))
            },
            block,
        })
    }

    #[generate_interface]
    pub fn exists(&self, trx: &mut WorkspaceTransaction, block_id: &str) -> bool {
        trx.trx.get_blocks().exists(&trx.trx.trx, block_id)
    }

    #[generate_interface]
    pub fn with_trx(&self, on_trx: Box<dyn OnWorkspaceTransaction>) -> bool {
        self.workspace
            .try_with_trx(|trx| {
                on_trx.on_trx(WorkspaceTransaction {
                    trx,
                    jwst_ws: &self.jwst_workspace,
                })
            })
            .is_some()
    }

    #[generate_interface]
    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        self.workspace.with_trx(|mut trx| {
            trx.get_blocks()
                .get_blocks_by_flavour(&trx.trx, flavour)
                .iter()
                .map(|block| Block {
                    block: block.clone(),
                    jwst_workspace: self.jwst_workspace.clone(),
                    jwst_block: {
                        let mut ws = self.jwst_workspace.clone();
                        ws.as_mut()
                            .and_then(|ws| ws.get_blocks().ok())
                            .and_then(|b| b.get(block.block_id()))
                    },
                })
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

    pub fn compare(self: &mut Workspace) -> Option<String> {
        if let Some(jwst_workspace) = self.jwst_workspace.as_mut() {
            match self
                .workspace
                .retry_with_trx(|trx| workspace_compare(trx.trx, jwst_workspace, None), 50)
            {
                Ok(ret) => {
                    self.runtime.block_on(async {
                        if let Err(e) = self.sender.send(Log::new(self.workspace.id(), ret.clone())).await {
                            warn!("failed to send log: {}", e);
                        }
                    });
                    return Some(ret);
                }
                Err(e) => {
                    warn!("failed to compare: {}", e);
                }
            }
        }
        None
    }
}
