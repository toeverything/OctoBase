use android_logger::Config;
use jwst::{error, DocStorage, DocSync};
use jwst_storage::DocAutoStorage;
use log::LevelFilter;
use std::{
    io::Result,
    sync::{Arc, RwLock},
};
use tokio::runtime::Runtime;
use yrs::{updates::decoder::Decode, Doc, Update};

#[derive(Clone)]
pub struct JwstStorage {
    storage: Option<Arc<RwLock<DocAutoStorage>>>,
    error: Option<String>,
}

impl JwstStorage {
    pub fn new(path: String) -> Self {
        android_logger::init_once(
            Config::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag("jwst"),
        );

        let rt = Runtime::new().unwrap();

        match rt.block_on(DocAutoStorage::init_pool(&format!(
            "sqlite:{path}?mode=rwc"
        ))) {
            Ok(pool) => Self {
                storage: Some(Arc::new(RwLock::new(pool))),
                error: None,
            },
            Err(e) => Self {
                storage: None,
                error: Some(e.to_string()),
            },
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    pub fn connect(&self, workspace_id: String, remote: String) -> Result<()> {
        if let Some(storage) = &self.storage {
            let storage = storage.read().unwrap();
            let rt = Runtime::new().unwrap();
            rt.block_on(storage.sync(workspace_id, remote))?;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Storage not initialized",
            ))
        }
    }

    pub fn reload(&self, workspace_id: String, doc: &Doc) {
        if let Some(storage) = &self.storage {
            let storage = storage.write().unwrap();
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let updates = storage
                    .all(&workspace_id)
                    .await
                    .expect("Failed to get all updates");
                if !updates.is_empty() {
                    let mut trx = doc.transact();
                    for update in updates {
                        if let Ok(update) = Update::decode_v1(&update.blob) {
                            trx.apply_update(update);
                        } else {
                            error!("Failed to decode update: {}", update.timestamp);
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
