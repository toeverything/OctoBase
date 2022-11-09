use super::{generate_interface, JwstBlock, Workspace};

pub struct Block(pub(crate) JwstBlock);

impl Block {
    #[generate_interface(constructor)]
    pub fn new(workspace: &Workspace, block_id: String, flavor: String, operator: u64) -> Block {
        let mut trx = workspace.get_trx().0.trx;
        Self(JwstBlock::new(
            &workspace.0,
            &mut trx,
            block_id,
            flavor,
            operator,
        ))
    }

    #[generate_interface]
    pub fn id(&self) -> String {
        self.0.id()
    }

    #[generate_interface]
    pub fn flavor(&self) -> String {
        self.0.flavor()
    }

    #[generate_interface]
    pub fn version(&self) -> String {
        let [major, minor] = self.0.version();
        format!("{}.{}", major, minor)
    }

    #[generate_interface]
    pub fn created(&self) -> u64 {
        self.0.created()
    }

    #[generate_interface]
    pub fn updated(&self) -> u64 {
        self.0.updated()
    }

    #[generate_interface]
    pub fn parent(&self) -> Option<String> {
        self.0.parent()
    }

    #[generate_interface]
    pub fn children(&self) -> Vec<String> {
        self.0.children()
    }
}
