use android_logger::Config;
use jwst::{error, DocStorage};
use jwst_storage::DocSQLiteStorage;
use log::Level;
use sqlx::Error;
use std::{
    io::Result,
    sync::{Arc, RwLock},
};
use tokio::runtime::Runtime;
use yrs::{updates::decoder::Decode, Doc, Update};

#[derive(Clone)]
pub struct JwstStorage {
    storage: Option<Arc<RwLock<DocSQLiteStorage>>>,
    error: Option<String>,
}

impl JwstStorage {
    pub fn new(path: String) -> Self {
        android_logger::init_once(
            Config::default()
                .with_min_level(Level::Info)
                .with_tag("jwst"),
        );

        let rt = Runtime::new().unwrap();

        match rt.block_on(DocSQLiteStorage::init_absolute_pool(&path)) {
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

    pub fn reload(&self, workspace_id: String, doc: &Doc) {
        if let Some(storage) = &self.storage {
            let storage = storage.write().unwrap();
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                storage
                    .create(&workspace_id)
                    .await
                    .expect("Failed to create workspace");
                let updates = storage
                    .all(&workspace_id)
                    .await
                    .expect("Failed to get all updates");
                if updates.len() > 0 {
                    let mut trx = doc.transact();
                    for update in updates {
                        if let Ok(update) = Update::decode_v1(&update.blob) {
                            trx.apply_update(update);
                        } else {
                            error!("Failed to decode update: {}", update.id);
                        }
                    }
                }
            });
        }
    }

    pub fn write_update(&self, workspace_id: String, update: &[u8]) -> Result<()> {
        if let Some(storage) = &self.storage {
            let storage = storage.write().unwrap();
            let rt = Runtime::new().unwrap();
            log::info!("update: {:?}", update);
            rt.block_on(storage.write_update(workspace_id, update))?;
        }
        Ok(())
    }
}
