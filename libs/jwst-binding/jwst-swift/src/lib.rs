use jwst::{Block as JwstBlock, Workspace as JwstWorkspace, WorkspaceTransaction};
use lib0::any::Any;
use yrs::{Subscription, Transaction, UpdateEvent};

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type Block;
        type Workspace;

        #[swift_bridge(init)]
        fn new(id: String) -> Workspace;

        #[swift_bridge(associated_to = Workspace)]
        fn get(self: &Workspace, block_id: String) -> Option<Block>;

        #[swift_bridge(associated_to = Workspace)]
        fn create(self: &Workspace, block_id: String, flavor: String) -> Block;

        #[swift_bridge(associated_to = Block)]
        fn to_string(self: &Block) -> String;
    }
}

pub struct Workspace {
    workspace: JwstWorkspace,
}

impl Workspace {
    fn new(id: String) -> Self {
        Self {
            workspace: JwstWorkspace::new(id),
        }
    }

    fn get(&self, block_id: String) -> Option<Block> {
        self.workspace.get(&block_id).map(Block::new)
    }

    fn create(&self, block_id: String, flavor: String) -> Block {
        self.workspace
            .with_trx(|trx| Block::new(trx.create(block_id, flavor)))
    }
}

pub struct Block {
    block: JwstBlock,
}

impl Block {
    fn new(block: JwstBlock) -> Self {
        Self { block }
    }

    // fn get(&self, key: String) -> Option<String> {
    //     self.block.get(&key)
    // }

    fn to_string(&self) -> String {
        format!("{:?}", self.block)
    }
}
