use super::*;
use jwst::{DocStorage, Workspace};
use jwst_storage::{BlobStorageType, JwstStorage};
use std::{collections::HashMap, time::Duration};
use tokio::{sync::RwLock, time::sleep};
use yrs::{ReadTxn, StateVector, Transact};

pub struct MinimumServerContext {
    channel: BroadcastChannels,
    storage: JwstStorage,
}

// just for test
impl MinimumServerContext {
    pub async fn new() -> Arc<Self> {
        let storage = 'connect: loop {
            let mut retry = 3;
            match JwstStorage::new_with_migration(
                &std::env::var("DATABASE_URL")
                    .map(|url| format!("{url}_binary"))
                    .unwrap_or("sqlite::memory:".into()),
                BlobStorageType::DB,
            )
            .await
            {
                Ok(storage) => break 'connect Ok(storage),
                Err(e) => {
                    retry -= 1;
                    if retry > 0 {
                        error!("failed to connect database: {}", e);
                        sleep(Duration::from_secs(1)).await;
                    } else {
                        break 'connect Err(e);
                    }
                }
            }
        }
        .unwrap();

        Arc::new(Self {
            channel: RwLock::new(HashMap::new()),
            storage,
        })
    }

    pub async fn new_with_workspace(workspace_id: &str) -> (Arc<MinimumServerContext>, Workspace, Vec<u8>) {
        let server = Self::new().await;
        server
            .get_storage()
            .docs()
            .delete_workspace(workspace_id)
            .await
            .unwrap();
        let ws = server.get_workspace(workspace_id).await.unwrap();

        let init_state = ws
            .doc()
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
            .expect("encode_state_as_update_v1 failed");

        (server, ws, init_state)
    }
}

impl RpcContextImpl<'_> for MinimumServerContext {
    fn get_storage(&self) -> &JwstStorage {
        &self.storage
    }

    fn get_channel(&self) -> &BroadcastChannels {
        &self.channel
    }
}
