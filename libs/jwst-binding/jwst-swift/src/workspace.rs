use super::Block;
use jwst::Workspace as JwstWorkspace;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
}

impl Workspace {
    pub fn new(id: String) -> Self {
        Self {
            workspace: JwstWorkspace::new(id),
        }
    }

    pub fn id(&self) -> String {
        self.workspace.id()
    }

    pub fn client_id(&self) -> u64 {
        self.workspace.client_id()
    }

    pub fn get(&self, block_id: String) -> Option<Block> {
        let workspace = self.workspace.clone();
        self.workspace.with_trx(|mut trx| {
            let block = trx
                .get_blocks()
                .get(&trx.trx, &block_id)
                .map(|b| Block::new(workspace, b));
            drop(trx);
            block
        })
    }

    pub fn create(&self, block_id: String, flavour: String) -> Block {
        let workspace = self.workspace.clone();
        self.workspace.with_trx(|mut trx| {
            let block = Block::new(
                workspace,
                trx.get_blocks()
                    .create(&mut trx.trx, block_id, flavour)
                    .expect("failed to create block"),
            );
            drop(trx);
            block
        })
    }

    pub fn search(self: &Workspace, query: String) -> String {
        self.workspace.search_result(query)
    }

    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        self.workspace
            .with_trx(|mut trx| trx.get_blocks().get_blocks_by_flavour(&trx.trx, flavour))
            .iter()
            .map(|block| Block {
                workspace: self.workspace.clone(),
                block: block.clone(),
            })
            .collect()
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
