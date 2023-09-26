use std::sync::Arc;

use tokio::runtime::Runtime;

use super::*;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    #[allow(dead_code)]
    pub(crate) runtime: Arc<Runtime>,
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
    pub fn create(&mut self, block_id: String, flavour: String) -> Block {
        self.workspace
            .get_blocks()
            .and_then(|mut s| s.create(&block_id, &flavour))
            .map(Block::from)
            .expect("failed to create block")
    }

    #[generate_interface]
    pub fn get(&mut self, block_id: String) -> Option<Block> {
        self.workspace
            .get_blocks()
            .ok()
            .and_then(|s| s.get(block_id))
            .map(Block::from)
    }

    #[generate_interface]
    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        self.workspace
            .clone()
            .get_blocks()
            .map(|s| s.get_blocks_by_flavour(flavour).into_iter().map(Block::from).collect())
            .unwrap_or_default()
    }

    #[generate_interface]
    pub fn exists(&self, block_id: &str) -> bool {
        self.workspace
            .clone()
            .get_blocks()
            .map(|s| s.exists(block_id))
            .unwrap_or(false)
    }

    #[generate_interface]
    pub fn remove(&mut self, block_id: String) -> bool {
        self.workspace.get_blocks().map(|mut s| s.remove(&block_id)).is_ok()
    }

    #[generate_interface]
    pub fn search(&self, _query: String) -> String {
        // self.workspace.search_result(query)
        "".to_owned()
    }

    #[generate_interface]
    pub fn get_search_index(&self) -> Vec<String> {
        // self.workspace.metadata().search_index
        vec![]
    }

    #[generate_interface]
    pub fn set_search_index(&self, _fields: VecOfStrings) -> bool {
        // self.workspace
        //     .set_search_index(fields)
        //     .expect("failed to set search index")
        false
    }

    // #[generate_interface]
    // pub fn set_callback(&self, observer: Box<dyn BlockObserver>) -> bool {
    //     // let observer = BlockObserverWrapper::new(observer);
    //     // self.workspace
    //     //     .set_callback(Arc::new(Box::new(move |_workspace_id, block_ids| {
    //     //         observer.on_change(block_ids);
    //     //     })));
    //     false
    // }
}
