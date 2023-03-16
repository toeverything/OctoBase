use super::Block;
use jwst::Workspace as JwstWorkspace;
use yrs::UpdateSubscription;

pub struct Workspace {
    pub(crate) workspace: JwstWorkspace,
    pub(crate) _sub: Option<UpdateSubscription>,
}

impl Workspace {
    pub fn new(id: String) -> Self {
        Self {
            workspace: JwstWorkspace::new(id),
            _sub: None,
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
                .get_space("blocks")
                .get(&trx.trx, &block_id)
                .map(|b| Block::new(workspace, b));
            drop(trx);
            block
        })
    }

    pub fn create(&self, block_id: String, flavor: String) -> Block {
        let workspace = self.workspace.clone();
        self.workspace.with_trx(|mut trx| {
            let block = Block::new(
                workspace,
                trx.get_space("blocks")
                    .create(&mut trx.trx, block_id, flavor),
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
            .with_trx(|mut trx| {
                trx.get_space("blocks")
                    .get_blocks_by_flavour(&trx.trx, flavour)
            })
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
        self.workspace.set_search_index(fields)
    }
}
