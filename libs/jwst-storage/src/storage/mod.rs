pub(crate) mod blobs;
mod docs;
mod test;

use super::*;
use crate::types::JwstStorageError;
use blobs::BlobAutoStorage;
use docs::SharedDocDBStorage;
use std::{collections::HashMap, time::Instant};
use tokio::sync::Mutex;

pub struct JwstStorage {
    pool: DatabaseConnection,
    blobs: BlobAutoStorage,
    docs: SharedDocDBStorage,
    last_migrate: Mutex<HashMap<String, Instant>>,
}

impl JwstStorage {
    pub async fn new(database: &str) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;
        let bucket = get_bucket(is_sqlite);

        if is_sqlite {
            pool.execute_unprepared("PRAGMA journal_mode=WAL;")
                .await
                .unwrap();
        }

        let blobs = BlobAutoStorage::init_with_pool(pool.clone(), bucket.clone()).await?;
        let docs = SharedDocDBStorage::init_with_pool(pool.clone(), bucket.clone()).await?;

        Ok(Self {
            pool,
            blobs,
            docs,
            last_migrate: Mutex::new(HashMap::new()),
        })
    }

    pub async fn new_with_sqlite(file: &str) -> JwstStorageResult<Self> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).map_err(JwstStorageError::CreateDataFolder)?;
        }

        Self::new(&format!(
            "sqlite:{}?mode=rwc",
            data.join(PathBuf::from(file).name_str())
                .with_extension("db")
                .display()
        ))
        .await
    }

    pub fn database(&self) -> String {
        format!("{:?}", self.pool)
    }

    pub fn blobs(&self) -> &BlobAutoStorage {
        &self.blobs
    }

    pub fn docs(&self) -> &SharedDocDBStorage {
        &self.docs
    }

    pub async fn with_pool<R, F, Fut>(&self, func: F) -> JwstStorageResult<R>
    where
        F: Fn(DatabaseConnection) -> Fut,
        Fut: Future<Output = JwstStorageResult<R>>,
    {
        func(self.pool.clone()).await
    }

    pub async fn create_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        info!("create_workspace: {}", workspace_id.as_ref());

        Ok(self
            .docs
            .get(workspace_id.as_ref().into())
            .await
            .map_err(|_err| {
                JwstStorageError::Crud(format!(
                    "Failed to create workspace {}",
                    workspace_id.as_ref()
                ))
            })?)
    }

    pub async fn get_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        trace!("get_workspace: {}", workspace_id.as_ref());
        if self
            .docs
            .exists(workspace_id.as_ref().into())
            .await
            .map_err(|_err| {
                JwstStorageError::Crud(format!(
                    "failed to check workspace {}",
                    workspace_id.as_ref()
                ))
            })?
        {
            Ok(self
                .docs
                .get(workspace_id.as_ref().into())
                .await
                .map_err(|_err| {
                    JwstStorageError::Crud(format!(
                        "failed to get workspace {}",
                        workspace_id.as_ref()
                    ))
                })?)
        } else {
            Err(JwstStorageError::WorkspaceNotFound(
                workspace_id.as_ref().into(),
            ))
        }
    }

    pub async fn full_migrate(
        &self,
        workspace_id: String,
        update: Option<Vec<u8>>,
        force: bool,
    ) -> bool {
        let mut map = self.last_migrate.lock().await;
        let ts = map.entry(workspace_id.clone()).or_insert(Instant::now());

        if ts.elapsed().as_secs() > 5 || force {
            debug!("full migrate: {workspace_id}");
            match self.docs.get(workspace_id.clone()).await {
                Ok(workspace) => {
                    let update = if let Some(update) = update {
                        if let Err(e) = self.docs.delete(workspace_id.clone()).await {
                            error!("full_migrate write error: {}", e.to_string());
                            return false;
                        };
                        Some(update)
                    } else {
                        workspace.sync_migration(50).ok()
                    };

                    let Some(update) = update else {
                        error!("full migrate failed: wait transact timeout");
                        return false;
                    };
                    if let Err(e) = self
                        .docs
                        .write_full_update(workspace_id.clone(), update)
                        .await
                    {
                        error!("db write error: {}", e.to_string());
                        return false;
                    }

                    *ts = Instant::now();

                    info!("full migrate final: {workspace_id}");
                    true
                }
                Err(e) => {
                    warn!("workspace {workspace_id} not exists in cache: {e:?}");
                    false
                }
            }
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_sqlite_storage() {
        let storage = JwstStorage::new_with_sqlite(":memory:").await.unwrap();
        assert_eq!(storage.database(), "SqlxSqlitePoolConnection");
    }
}
