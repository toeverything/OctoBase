use super::*;
use chrono::{DateTime, Utc};
use jwst_rpc::workspace_compare;
use jwst_storage::JwstStorage;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    runtime::Runtime,
    sync::{mpsc::Receiver, Mutex},
    time::sleep,
};

#[derive(Clone, Debug, Serialize)]
pub struct Log {
    content: String,
    timestamp: DateTime<Utc>,
    workspace: String,
}

impl Log {
    pub fn new(workspace: String, content: String) -> Self {
        Self {
            content,
            timestamp: chrono::Utc::now(),
            workspace,
        }
    }
}

#[derive(Clone, Default)]
pub struct CachedDiffLog {
    synced: Arc<Mutex<Vec<Log>>>,
}

impl CachedDiffLog {
    pub fn add_receiver(
        &self,
        mut receiver: Receiver<Log>,
        rt: Arc<Runtime>,
        storage: Arc<JwstStorage>,
        workspace: jwst::Workspace,
        jwst_workspace: jwst_core::Workspace,
    ) {
        let synced = self.synced.clone();

        rt.spawn(async move {
            loop {
                tokio::select! {
                    Some(last_synced) = receiver.recv() => {
                        let mut synced = synced.lock().await;
                        synced.push(last_synced);
                    }
                    _ = sleep(Duration::from_secs(5)) => {
                        let mut synced = synced.lock().await;
                        for log in synced.iter() {
                            if let Err(e) = storage
                                .difflog()
                                .insert(log.workspace.clone(), log.timestamp, log.content.clone())
                                .await
                            {
                                error!("failed to insert diff log: {:?}", e);
                            }
                        }
                        synced.clear();

                        let ret = {
                            let workspace = workspace.clone();
                            let mut jwst_workspace = jwst_workspace.clone();
                            match tokio::task::spawn_blocking(move || {
                                workspace_compare(&workspace, &mut jwst_workspace)
                            })
                            .await
                            {
                                Ok(ret) => ret,
                                Err(e) => {
                                    format!("failed to compare workspace: {}", e)
                                }
                            }
                        };
                        if let Err(e) = storage
                            .difflog()
                            .insert(workspace.id(), chrono::Utc::now(), ret)
                            .await
                        {
                            error!("failed to insert diff log: {:?}", e);
                        }
                    }
                }
            }
        });
    }
}
