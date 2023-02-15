use android_logger::Config;
use jwst::{error, DocStorage, DocSync};
use jwst_storage::JwstStorage as AutoStorage;
use log::LevelFilter;
use std::{io::Result, sync::Arc};
use tokio::{runtime::Runtime, sync::RwLock};
use yrs::{updates::decoder::Decode, Doc, Update};

#[derive(Clone)]
pub struct JwstStorage {
    storage: Option<Arc<RwLock<AutoStorage>>>,
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

        match rt.block_on(AutoStorage::new(&format!("sqlite:{path}?mode=rwc"))) {
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
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                let storage = storage.read().await;
                storage.docs().sync(workspace_id, remote).await
            })?;
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
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                let storage = storage.write().await;
                let updates = storage
                    .docs()
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
            let rt = Runtime::new().unwrap();
            log::info!("update: {:?}", update);
            rt.block_on(async move {
                let storage = storage.write().await;
                storage.docs().write_update(workspace_id, update).await
            })?;
        }
        Ok(())
    }
}
