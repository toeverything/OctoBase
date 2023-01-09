use jwst::DocStorage;
use jwst_storage::DocSQLiteStorage;
use std::{
    io::Result,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct JwstStorage(Arc<RwLock<DocSQLiteStorage>>);

impl JwstStorage {
    pub fn write_update(&self, workspace_id: String, update: &[u8]) -> Result<()> {
        let storage = self.0.write().unwrap();
        futures::executor::block_on(storage.write_update(workspace_id, update))?;
        Ok(())
    }
}
