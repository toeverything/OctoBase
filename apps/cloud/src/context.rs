use super::{api::UserChannel, config::Config, utils::create_debug_collaboration_workspace};
use crate::application::blob_service::BlobService;
use cloud_database::CloudDatabase;
use cloud_infra::{FirebaseContext, KeyContext, MailContext};
use jwst::SearchResults;
use jwst_logger::{error, info, warn};
use jwst_rpc::{BroadcastChannels, BroadcastType, RpcContextImpl};
use jwst_storage::{BlobStorageType, JwstStorage, MixedBucketDBParam};
use std::{collections::HashMap, path::Path};
use tempfile::{tempdir, TempDir};
use tokio::sync::{Mutex, RwLock};

pub struct Context {
    pub key: KeyContext,
    pub firebase: Mutex<FirebaseContext>,
    pub mail: MailContext,
    pub db: CloudDatabase,
    pub storage: JwstStorage,
    pub user_channel: UserChannel,
    pub channel: BroadcastChannels,
    pub config: Config,
    _dir: Option<TempDir>,
    pub blob_service: BlobService,
}

impl Context {
    pub async fn new() -> Context {
        let (_dir, cloud, storage) = if let Ok(url) = dotenvy::var("DATABASE_URL") {
            (None, url.clone(), format!("{url}_binary"))
        } else {
            let (path, dir) = if let Ok(dir) = dotenvy::var("DATABASE_DIR") {
                let dir = Path::new(&dir);
                if !dir.try_exists().unwrap_or(false) {
                    std::fs::create_dir(dir).expect("Cannot create dir");
                }
                (dir.to_path_buf(), None)
            } else {
                let dir = tempdir().expect("Cannot create temp dir");
                warn!("!!! Be careful, the data is stored in a temporary directory !!!");
                (dir.path().to_path_buf(), Some(dir))
            };

            warn!("!!! Please set `DATABASE_URL` in .env file or environmental variable to save your data !!!");
            info!("!!! The data is stored in {} !!!", path.display());
            let cloud = format!("sqlite:{}?mode=rwc", path.join("cloud.db").display());
            let storage = format!("sqlite:{}?mode=rwc", path.join("storage.db").display());

            (dir, cloud, storage)
        };

        let db = CloudDatabase::init_pool(&cloud)
            .await
            .expect("Cannot create cloud database");

        let use_bucket_storage =
            dotenvy::var("ENABLE_BUCKET_STORAGE").map_or(false, |v| v.eq("true"));

        let storage_type = if use_bucket_storage {
            info!("use database and s3 bucket as blob storage");
            BlobStorageType::MixedBucketDB(
                MixedBucketDBParam::new_from_env()
                    .map_err(|e| format!("failed to load bucket param from env: {}", e))
                    .unwrap(),
            )
        } else {
            info!("use database as blob storage");
            BlobStorageType::DB
        };

        let storage = JwstStorage::new_with_migration(&storage, storage_type)
            .await
            .expect("Cannot create storage");

        let blob_service = BlobService::new();

        if cfg!(debug_assertions) || std::env::var("JWST_DEV").is_ok() {
            create_debug_collaboration_workspace(&db, &storage).await;
        }

        Self {
            _dir,
            // =========== database ===========
            db,
            storage,
            // =========== auth ===========
            key: KeyContext::new(dotenvy::var("SIGN_KEY").ok()).expect("Cannot create key context"),
            firebase: Mutex::new(FirebaseContext::new(
                dotenvy::var("FIREBASE_PROJECT_ID")
                    .map(|id| vec![id])
                    .unwrap_or_else(|_| {
                        vec!["pathfinder-52392".into(), "quiet-sanctuary-370417".into()]
                    }),
            )),
            // =========== mail ===========
            mail: MailContext::new(
                dotenvy::var("MAIL_ACCOUNT").ok(),
                dotenvy::var("MAIL_PASSWORD").ok(),
            ),
            // =========== sync channel ===========
            channel: RwLock::new(HashMap::new()),
            user_channel: UserChannel::new(),
            config: Config::new(),
            blob_service,
        }
    }

    #[cfg(test)]
    pub(super) async fn new_test_client(db: CloudDatabase) -> Self {
        Self {
            // =========== database ===========
            db,
            ..Self::new().await
        }
    }

    pub async fn search_workspace(
        &self,
        workspace_id: String,
        query_string: &str,
    ) -> Result<SearchResults, Box<dyn std::error::Error>> {
        let workspace_id = workspace_id.to_string();

        match self.storage.get_workspace(workspace_id.clone()).await {
            Ok(workspace) => {
                let search_results = workspace.search(query_string)?;
                Ok(search_results)
            }
            Err(e) => {
                error!("cannot get workspace: {:?}", e);
                Err(Box::new(e))
            }
        }
    }

    // TODO: this should be moved to another module
    pub async fn close_websocket(&self, workspace: String, user: String) {
        let mut closed = vec![];
        let event = BroadcastType::CloseUser(user);
        for (channel, tx) in self.channel.read().await.iter() {
            if channel == &workspace {
                if tx.receiver_count() <= 1 {
                    closed.push(channel.clone());
                }
                let _ = tx.send(event.clone());
            }
        }
        for channel in closed {
            self.channel.write().await.remove(&channel);
        }
    }

    // TODO: this should be moved to another module
    pub async fn close_websocket_by_workspace(&self, workspace: String) {
        let mut closed = vec![];
        for (id, tx) in self.channel.read().await.iter() {
            if id == &workspace {
                closed.push(id.clone());
                let _ = tx.send(BroadcastType::CloseAll);
            }
        }
        for channel in closed {
            self.channel.write().await.remove(&channel);
        }
    }
}

impl RpcContextImpl<'_> for Context {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}
