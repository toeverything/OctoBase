use super::utils::calculate_hash;
use super::utils::get_hash;
use super::*;
use crate::rate_limiter::Bucket;
use crate::JwstStorageError;
use bytes::Bytes;
use futures::Stream;
use jwst::{BlobMetadata, BlobStorage, BucketBlobStorage, JwstResult};
use jwst_storage_migration::Migrator;
use opendal::services::S3;
use opendal::Operator;
use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm_migration::MigratorTrait;

use std::collections::HashMap;
use std::sync::Arc;

pub(super) type BucketBlobModel = <BucketBlobs as EntityTrait>::Model;
type BucketBlobActiveModel = entities::bucket_blobs::ActiveModel;
type BucketBlobColumn = <BucketBlobs as EntityTrait>::Column;

#[derive(Clone)]
pub struct BlobBucketDBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
    pub(super) bucket_storage: BucketStorage,
}

impl AsRef<DatabaseConnection> for BlobBucketDBStorage {
    fn as_ref(&self) -> &DatabaseConnection {
        &self.pool
    }
}

impl BlobBucketDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
        bucket_storage: Option<BucketStorage>,
    ) -> JwstStorageResult<Self> {
        Migrator::up(&pool, None).await?;
        Ok(Self {
            bucket,
            pool,
            bucket_storage: bucket_storage.unwrap_or(BucketStorage::new()?),
        })
    }

    #[allow(unused)]
    pub async fn init_pool(
        database: &str,
        bucket_storage: Option<BucketStorage>,
    ) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite), bucket_storage).await
    }

    #[allow(unused)]
    async fn all(&self, workspace: &str) -> Result<Vec<BucketBlobModel>, DbErr> {
        BucketBlobs::find()
            .filter(BucketBlobColumn::WorkspaceId.eq(workspace))
            .all(&self.pool)
            .await
    }

    async fn keys(&self, workspace: &str) -> Result<Vec<String>, DbErr> {
        BucketBlobs::find()
            .filter(BucketBlobColumn::WorkspaceId.eq(workspace))
            .column_as(BucketBlobColumn::Hash, "hash")
            .all(&self.pool)
            .await
            .map(|r| r.into_iter().map(|f| f.hash).collect())
    }

    #[allow(unused)]
    async fn count(&self, workspace: &str) -> Result<u64, DbErr> {
        BucketBlobs::find()
            .filter(BucketBlobColumn::WorkspaceId.eq(workspace))
            .count(&self.pool)
            .await
    }

    async fn exists(&self, workspace: &str, hash: &str) -> Result<bool, DbErr> {
        BucketBlobs::find_by_id((workspace.into(), hash.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)
    }

    pub(super) async fn metadata(
        &self,
        workspace: &str,
        hash: &str,
    ) -> JwstBlobResult<InternalBlobMetadata> {
        BucketBlobs::find_by_id((workspace.into(), hash.into()))
            .select_only()
            .column_as(BucketBlobColumn::Length, "size")
            .column_as(BucketBlobColumn::CreatedAt, "created_at")
            .into_model::<InternalBlobMetadata>()
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn get_blobs_size(&self, workspace: &str) -> Result<Option<i64>, DbErr> {
        BucketBlobs::find()
            .filter(BucketBlobColumn::WorkspaceId.eq(workspace))
            .column_as(BucketBlobColumn::Length, "size")
            .column_as(BucketBlobColumn::CreatedAt, "created_at")
            .into_model::<InternalBlobMetadata>()
            .all(&self.pool)
            .await
            .map(|r| r.into_iter().map(|f| f.size).reduce(|a, b| a + b))
    }

    async fn insert(&self, workspace: &str, hash: &str, blob: &[u8]) -> Result<(), DbErr> {
        if !self.exists(workspace, hash).await? {
            BucketBlobs::insert(BucketBlobActiveModel {
                workspace_id: Set(workspace.into()),
                hash: Set(hash.into()),
                length: Set(blob.len().try_into().unwrap()),
                created_at: Set(Utc::now().into()),
            })
            .exec(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn delete(&self, workspace: &str, hash: &str) -> Result<bool, DbErr> {
        BucketBlobs::delete_by_id((workspace.into(), hash.into()))
            .exec(&self.pool)
            .await
            .map(|r| r.rows_affected == 1)
    }

    async fn drop(&self, workspace: &str) -> Result<(), DbErr> {
        BucketBlobs::delete_many()
            .filter(BucketBlobColumn::WorkspaceId.eq(workspace))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct BucketStorage {
    pub(super) op: Operator,
}

impl BucketStorage {
    #[allow(unused)]
    pub fn new() -> JwstStorageResult<Self> {
        let access_key = dotenvy::var("BUCKET_ACCESS_TOKEN")?;
        let secret_access_key = dotenvy::var("BUCKET_SECRET_TOKEN")?;
        let endpoint = dotenvy::var("BUCKET_ENDPOINT")?;
        let bucket = dotenvy::var("BUCKET_NAME");
        let root = dotenvy::var("BUCKET_ROOT");

        let mut builder = S3::default();

        builder.bucket(bucket.unwrap_or("__default_bucket__".to_string()).as_str());
        builder.root(root.unwrap_or("__default_root__".to_string()).as_str());
        builder.endpoint(endpoint.as_str());
        builder.access_key_id(access_key.as_str());
        builder.secret_access_key(secret_access_key.as_str());

        Ok(Self {
            op: Operator::new(builder)?.finish(),
        })
    }
}

#[async_trait]
impl BucketBlobStorage<JwstStorageError> for BucketStorage {
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        let workspace = get_workspace(workspace);
        let key = build_key(workspace, id);
        let bs = self.op.read(&key).await?;

        Ok(bs)
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        hash: String,
        blob: Vec<u8>,
    ) -> JwstResult<(), JwstStorageError> {
        let workspace = get_workspace(workspace);
        let key = build_key(workspace, hash);
        let _ = self.op.write(&key, blob).await?;

        Ok(())
    }

    async fn delete_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        let workspace = get_workspace(workspace);
        let key = build_key(workspace, id);

        match self.op.delete(&key).await {
            Ok(_) => Ok(true),
            Err(e) => Err(JwstStorageError::from(e)),
        }
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        self.op.remove_all(&workspace_id).await?;

        Ok(())
    }
}

#[async_trait]
impl BlobStorage<JwstStorageError> for BlobBucketDBStorage {
    async fn list_blobs(
        &self,
        workspace: Option<String>,
    ) -> JwstResult<Vec<String>, JwstStorageError> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(keys) = self.keys(&workspace).await {
            return Ok(keys);
        }

        Err(JwstStorageError::WorkspaceNotFound(workspace))
    }

    async fn check_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(exists) = self.exists(&workspace, &id).await {
            return Ok(exists);
        }

        Err(JwstStorageError::WorkspaceNotFound(workspace))
    }

    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        _params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        self.bucket_storage.get_blob(workspace, id).await
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        _params: Option<HashMap<String, String>>,
    ) -> JwstResult<BlobMetadata, JwstStorageError> {
        let _lock = self.bucket.read().await;
        let workspace = get_workspace(workspace);
        if let Ok(metadata) = self.metadata(&workspace, &id).await {
            Ok(metadata.into())
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn put_blob_stream(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, JwstStorageError> {
        let (hash, blob) = get_hash(stream).await;
        self.bucket_storage
            .put_blob(workspace.clone(), hash.clone(), blob.clone())
            .await?;
        let _lock = self.bucket.write().await;
        let workspace = get_workspace(workspace);

        if self.insert(&workspace, &hash, &blob).await.is_ok() {
            Ok(hash)
        } else {
            self.bucket_storage
                .delete_blob(Some(workspace.clone()), hash)
                .await?;
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        blob: Vec<u8>,
    ) -> JwstResult<String, JwstStorageError> {
        let hash = calculate_hash(&blob);
        self.bucket_storage
            .put_blob(workspace.clone(), hash.clone(), blob.clone())
            .await?;

        let _lock = self.bucket.write().await;
        let workspace = get_workspace(workspace);

        if self.insert(&workspace, &hash, &blob).await.is_ok() {
            Ok(hash)
        } else {
            self.bucket_storage
                .delete_blob(Some(workspace.clone()), hash)
                .await?;
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn delete_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        self.bucket_storage
            .delete_blob(workspace.clone(), id.clone())
            .await?;
        let _lock = self.bucket.write().await;
        let workspace = get_workspace(workspace);
        if let Ok(success) = self.delete(&workspace, &id).await {
            Ok(success)
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        self.bucket_storage
            .delete_workspace(workspace_id.clone())
            .await?;
        let _lock = self.bucket.write().await;
        if self.drop(&workspace_id).await.is_ok() {
            Ok(())
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace_id))
        }
    }

    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, JwstStorageError> {
        let _lock = self.bucket.read().await;
        let size = self.get_blobs_size(&workspace_id).await?;
        return Ok(size.unwrap_or(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::blobs::utils::BucketStorageBuilder;

    #[tokio::test]
    #[ignore = "need to config bucket auth"]
    async fn test_init_bucket_storage() {
        let bucket_storage = BucketStorageBuilder::new()
            .endpoint("ENDPOINT")
            .access_key("ACCESS_KEY")
            .secret_access_key("SECRET_ACCESS_KEY")
            .bucket("__default_bucket__")
            .root("__default_root__")
            .build()
            .unwrap();

        BlobBucketDBStorage::init_pool("sqlite::memory:", Some(bucket_storage))
            .await
            .unwrap();
    }
}

/// get_workspace will get the workspace name from the input.
///
/// If the input is None, it will return the default workspace name.
fn get_workspace(workspace: Option<String>) -> String {
    match workspace {
        Some(w) => w,
        None => "__default__".into(),
    }
}

/// build_key will build the request key for the bucket storage.
fn build_key(workspace: String, id: String) -> String {
    format!("{}/{}", workspace, id)
}
