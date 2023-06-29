pub(crate) mod blobs;
mod docs;
mod test;

use super::*;
use crate::storage::blobs::{BlobBucketDBStorage, BlobStorageType, JwstBlobStorage};
use crate::types::JwstStorageError;
use blobs::BlobAutoStorage;
use docs::SharedDocDBStorage;
use jwst_storage_migration::{Migrator, MigratorTrait};
use std::{collections::HashMap, time::Instant};
use tokio::sync::Mutex;

pub struct JwstStorage {
    pool: DatabaseConnection,
    blobs: JwstBlobStorage,
    docs: SharedDocDBStorage,
    last_migrate: Mutex<HashMap<String, Instant>>,
}

impl JwstStorage {
    pub async fn new(
        database: &str,
        blob_storage_type: BlobStorageType,
    ) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;
        let bucket = get_bucket(is_sqlite);

        if is_sqlite {
            pool.execute_unprepared("PRAGMA journal_mode=WAL;")
                .await
                .unwrap();
        }

        let blobs = match blob_storage_type {
            BlobStorageType::DB => JwstBlobStorage::DB(
                BlobAutoStorage::init_with_pool(pool.clone(), bucket.clone()).await?,
            ),
            BlobStorageType::MixedBucketDB(param) => JwstBlobStorage::MixedBucketDB(
                BlobBucketDBStorage::init_with_pool(
                    pool.clone(),
                    bucket.clone(),
                    Some(param.try_into()?),
                )
                .await?,
            ),
        };
        let docs = SharedDocDBStorage::init_with_pool(pool.clone(), bucket.clone()).await?;

        Ok(Self {
            pool,
            blobs,
            docs,
            last_migrate: Mutex::new(HashMap::new()),
        })
    }

    pub async fn new_with_migration(
        database: &str,
        blob_storage_type: BlobStorageType,
    ) -> JwstStorageResult<Self> {
        let storage = Self::new(database, blob_storage_type).await?;

        storage.db_migrate().await?;

        Ok(storage)
    }

    async fn db_migrate(&self) -> JwstStorageResult<()> {
        Migrator::up(&self.pool, None).await?;
        Ok(())
    }

    pub async fn new_with_sqlite(
        file: &str,
        blob_storage_type: BlobStorageType,
    ) -> JwstStorageResult<Self> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).map_err(JwstStorageError::CreateDataFolder)?;
        }

        Self::new_with_migration(
            &format!(
                "sqlite:{}?mode=rwc",
                data.join(PathBuf::from(file).name_str())
                    .with_extension("db")
                    .display()
            ),
            blob_storage_type,
        )
        .await
    }

    pub fn database(&self) -> String {
        format!("{:?}", self.pool)
    }

    pub fn blobs(&self) -> &JwstBlobStorage {
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

        self.docs
            .get_or_create_workspace(workspace_id.as_ref().into())
            .await
            .map_err(|_err| {
                JwstStorageError::Crud(format!(
                    "Failed to create workspace {}",
                    workspace_id.as_ref()
                ))
            })
    }

    pub async fn get_workspace<S>(&self, workspace_id: S) -> JwstStorageResult<Workspace>
    where
        S: AsRef<str>,
    {
        trace!("get_workspace: {}", workspace_id.as_ref());
        if self
            .docs
            .detect_workspace(workspace_id.as_ref())
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
                .get_or_create_workspace(workspace_id.as_ref().into())
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
            match self
                .docs
                .get_or_create_workspace(workspace_id.clone())
                .await
            {
                Ok(workspace) => {
                    let update = if let Some(update) = update {
                        if let Err(e) = self.docs.delete_workspace(&workspace_id).await {
                            error!("full_migrate write error: {}", e.to_string());
                            return false;
                        };
                        Some(update)
                    } else {
                        workspace.sync_migration().ok()
                    };

                    let Some(update) = update else {
                        error!("full migrate failed: wait transact timeout");
                        return false;
                    };
                    if let Err(e) = self
                        .docs
                        .flush_workspace(workspace_id.clone(), update)
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
    use crate::storage::blobs::MixedBucketDBParam;

    #[tokio::test]
    async fn test_sqlite_storage() {
        let storage = JwstStorage::new_with_sqlite(":memory:", BlobStorageType::DB)
            .await
            .unwrap();
        assert_eq!(storage.database(), "SqlxSqlitePoolConnection");
    }

    #[tokio::test]
    #[ignore = "need to config bucket auth"]
    async fn test_bucket_storage() {
        let bucket_params = MixedBucketDBParam {
            access_key: dotenvy::var("BUCKET_ACCESS_TOKEN").unwrap().to_string(),
            secret_access_key: dotenvy::var("BUCKET_SECRET_TOKEN").unwrap().to_string(),
            endpoint: dotenvy::var("BUCKET_ENDPOINT").unwrap().to_string(),
            bucket: Some(dotenvy::var("BUCKET_NAME").unwrap()),
            root: Some(dotenvy::var("BUCKET_ROOT").unwrap()),
        };
        let _storage =
            JwstStorage::new_with_sqlite(":memory:", BlobStorageType::MixedBucketDB(bucket_params))
                .await
                .unwrap();
    }
}
