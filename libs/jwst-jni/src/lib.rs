mod java_glue;

pub use crate::java_glue::*;

use jwst::{Block as JwstBlock, Workspace as JwstWorkspace};
use rifgen::rifgen_attr::*;

pub struct Workspace(JwstWorkspace);

impl Workspace {
    #[generate_interface(constructor)]
    pub fn new(id: String) -> Workspace {
        Self(JwstWorkspace::new(id))
    }
}

pub struct Block(JwstBlock);
