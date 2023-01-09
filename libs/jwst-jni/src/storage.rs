use futures::executor::block_on;
use jwst::DocStorage;
use jwst_storage::DocSQLiteStorage;
use sqlx::Error;
use std::{
    io::Result,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct JwstStorage {
    storage: Option<Arc<RwLock<DocSQLiteStorage>>>,
    error: Option<String>,
}

impl JwstStorage {
    pub fn new(path: String) -> Self {
        match block_on(DocSQLiteStorage::init_pool(&path)) {
            Ok(pool) => Self {
                storage: Some(Arc::new(RwLock::new(pool))),
                error: None,
            },
            Err(e) => Self {
                storage: None,
                error: Some(match e {
                    Error::Io(e) => e.to_string(),
                    _ => e.to_string(),
                }),
            },
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    pub fn write_update(&self, workspace_id: String, update: &[u8]) -> Result<()> {
        if let Some(doc) = &self.storage {
            let storage = doc.write().unwrap();
            block_on(storage.write_update(workspace_id, update))?;
        }
        Ok(())
    }
}
