use std::sync::Arc;

use jwst_core::Workspace as JwstWorkspace;
use tokio::runtime::Runtime;

use super::*;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) rt: Arc<Runtime>,
}

impl Workspace {
    pub fn new(id: String) -> Self {
        Self {
            workspace: JwstWorkspace::new(&id).unwrap(),
            rt: Arc::new(Runtime::new().unwrap()),
        }
    }

    pub fn id(&self) -> String {
        self.workspace.id()
    }

    pub fn client_id(&self) -> u64 {
        self.workspace.client_id()
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        let mut workspace = self.workspace.clone();

        workspace
            .get_blocks()
            .ok()
            .and_then(|s| s.get(&block_id))
            .map(Block::new)
    }

    pub fn create(&self, block_id: String, flavour: String) -> Block {
        let mut workspace = self.workspace.clone();

        Block::new(
            workspace
                .get_blocks()
                .and_then(|mut b| b.create(block_id, flavour))
                .expect("failed to create jwst block"),
        )
    }

    pub fn search(self: &Workspace, query: String) -> String {
        // self.workspace.search_result(query)
        "".to_owned()
    }

    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        let mut workspace = self.workspace.clone();

        workspace
            .get_blocks()
            .map(|s| s.get_blocks_by_flavour(flavour).into_iter().map(Block::new).collect())
            .unwrap_or_default()
    }

    pub fn get_search_index(self: &Workspace) -> Vec<String> {
        // self.workspace.metadata().search_index
        vec![]
    }

    pub fn set_search_index(self: &Workspace, fields: Vec<String>) -> bool {
        // self.workspace
        //     .set_search_index(fields)
        //     .expect("failed to set search index")
        false
    }
}
