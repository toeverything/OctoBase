use cloud_components::MailContext;
use cloud_database::CloudDatabase;
use jwst::SearchResults;
use jwst_logger::error;
use jwst_rpc::{BroadcastChannels, BroadcastType, ContextImpl};
use jwst_storage::JwstStorage;
use std::collections::HashMap;
use tokio::sync::{Mutex, RwLock};

use crate::api::UserChannel;
use crate::key::{FirebaseContext, KeyContext};

pub struct Context {
    pub key: KeyContext,
    pub site_url: String,
    pub firebase: Mutex<FirebaseContext>,
    pub mail: MailContext,
    pub db: CloudDatabase,
    pub storage: JwstStorage,
    pub user_channel: UserChannel,
    pub channel: BroadcastChannels,
}

impl Context {
    pub async fn new() -> Context {
        let site_url = dotenvy::var("SITE_URL").expect("should provide site url");

        Self {
            // =========== database ===========
            db: CloudDatabase::init_pool(
                dotenvy::var("DATABASE_URL")
                    .as_deref()
                    .unwrap_or("sqlite://affine.db?mode=rwc"),
            )
            .await
            .expect("Cannot create cloud database"),
            storage: JwstStorage::new(
                dotenvy::var("DATABASE_URL")
                    .map(|db| format!("{db}_binary"))
                    .as_deref()
                    .unwrap_or("sqlite://affine.binary.db?mode=rwc"),
            )
            .await
            .expect("Cannot create storage"),
            // =========== auth ===========
            key: KeyContext::new(dotenvy::var("SIGN_KEY").expect("should provide AES key")),
            firebase: Mutex::new(FirebaseContext::new(
                dotenvy::var("FIREBASE_PROJECT_ID")
                    .map(|id| vec![id])
                    .unwrap_or_else(|_| {
                        vec!["pathfinder-52392".into(), "quiet-sanctuary-370417".into()]
                    }),
            )),
            // =========== mail ===========
            mail: MailContext::new(
                dotenvy::var("MAIL_ACCOUNT").expect("should provide email name"),
                dotenvy::var("MAIL_PASSWORD").expect("should provide email password"),
            ),
            site_url,
            // =========== sync channel ===========
            channel: RwLock::new(HashMap::new()),
            user_channel: UserChannel::new(),
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
                error!("cannot get workspace: {}", e);
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

impl ContextImpl<'_> for Context {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}
