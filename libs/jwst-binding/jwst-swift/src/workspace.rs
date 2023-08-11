use super::{Block, Log};
use jwst::Workspace as JwstWorkspace;
use jwst_rpc::workspace_compare;
use std::{sync::mpsc::Sender, sync::Arc};
use tokio::runtime::Runtime;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) jwst_workspace: Option<jwst_core::Workspace>,
    pub(crate) runtime: Arc<Runtime>,

    pub(crate) sender: Sender<Log>,
}

impl Workspace {
    pub fn new(id: String, runtime: Arc<Runtime>) -> Self {
        let (sender, _receiver) = std::sync::mpsc::channel();

        Self {
            workspace: JwstWorkspace::new(&id),
            jwst_workspace: jwst_core::Workspace::new(id).ok(),
            runtime,
            sender,
            // receiver,
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

            let jwst_workspace = self.jwst_workspace.clone();
            let sender = self.sender.clone();

            self.runtime
                .spawn(async move {
                    workspace.with_trx(|mut trx| {
                        let block = trx.get_blocks().get(&trx.trx, &block_id).map(|b| {
                            Block::new(workspace.clone(), b, runtime, jwst_workspace, sender)
                        });
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

            let jwst_workspace = self.jwst_workspace.clone();
            let sender = self.sender.clone();

            let runtime = self.runtime.clone();
            self.runtime
                .spawn(async move {
                    workspace.with_trx(|mut trx| {
                        let block = trx
                            .get_blocks()
                            .create(&mut trx.trx, block_id.clone(), flavour.clone())
                            .expect("failed to create block");

                        let created = block.created(&trx.trx);

                        let block = Block::new(
                            workspace.clone(),
                            block,
                            runtime,
                            {
                                // just for data verify
                                if let Some(mut jwst_workspace) = jwst_workspace.clone() {
                                    jwst_workspace
                                        .get_blocks()
                                        .and_then(|mut b| b.create_ffi(block_id, flavour, created))
                                        .expect("failed to create jwst block");

                                    // let ret = workspace_compare(&workspace, &jwst_workspace);
                                    // sender.send(Log::new(workspace.id(), ret)).unwrap();
                                }

                                jwst_workspace
                            },
                            sender.clone(),
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

            let jwst_workspace = self.jwst_workspace.clone();
            let sender = self.sender.clone();

            self.runtime
                .spawn(async move {
                    workspace
                        .with_trx(|mut trx| {
                            trx.get_blocks().get_blocks_by_flavour(&trx.trx, &flavour)
                        })
                        .iter()
                        .map(|block| {
                            Block::new(
                                workspace.clone(),
                                block.clone(),
                                runtime.clone(),
                                jwst_workspace.clone(),
                                sender.clone(),
                            )
                        })
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

    pub fn compare(self: &mut Workspace) -> Option<String> {
        if let Some(jwst_workspace) = self.jwst_workspace.as_mut() {
            let ret = workspace_compare(&self.workspace, jwst_workspace);
            self.sender
                .send(Log::new(self.workspace.id(), ret.clone()))
                .unwrap();
            Some(ret)
        } else {
            None
        }
    }
}
