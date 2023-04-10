use std::collections::HashSet;
use std::sync::{Arc, Mutex, RwLock};
use log::warn;
use super::{
    generate_interface, Block, JwstWorkspace, OnWorkspaceTransaction, VecOfStrings,
    WorkspaceTransaction,
};

pub trait BlockObserver {
    fn on_change(&self, block_id: String);
}

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) tx: std::sync::mpsc::Sender<String>,
    pub(crate) rx: Arc<Mutex<std::sync::mpsc::Receiver<String>>>,
    pub(crate) callback: Arc<Mutex<Option<Box<dyn BlockObserver>>>>,
    pub(crate) observed_block_ids: Arc<RwLock<HashSet<String>>>,
    pub(crate) modified_block_ids: Arc<RwLock<HashSet<String>>>,
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
    pub fn create_block(&self, block_id: String, flavour: String) -> Block {
        let mut block = self.workspace
            .with_trx(|mut trx| Block(trx.get_blocks().create(&mut trx.trx, block_id, flavour).unwrap()));

        let jwst_block = &mut block.0;
        jwst_block.subscribe(self.tx.clone());

        block
    }

    #[generate_interface]
    pub fn subscribe(&self, block_id: String, test: VecOfStrings) {
        let ids = vec!["1".to_string()];
        warn!("subscribe+++++++++++++++++++");
        self.some_fn(test);
    }

    fn some_fn(&self, test: Vec<String>) {
        warn!("some_fn+++++++++++++++++++{:?}", test);
    }

    fn produce_change(&self) {
        let rx = self.rx.clone();
        let modified_block_ids = self.modified_block_ids.clone();
        std::thread::spawn(move || {
            while let Ok(block_id) = (&*rx.lock().unwrap()).recv() {
                (&mut *modified_block_ids.write().unwrap()).insert(block_id);
            }
        });
    }

    #[generate_interface]
    pub fn set_callback(&mut self, callback: Box<dyn BlockObserver>) {
        let mut guard = self.callback.lock().unwrap();
        if let None = *guard {
            *guard = Some(callback);
        }
        println!("set_callback");
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
}
