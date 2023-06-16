use super::*;
use crate::rate_limiter::Bucket;
use crate::storage::blobs::utils::get_hash;
use crate::JwstStorageError;
use bytes::Bytes;
use futures::Stream;
use jwst::{BlobMetadata, BlobStorage, BucketBlobStorage, JwstResult};
use opendal::Operator;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::collections::HashMap;
use std::sync::Arc;

#[allow(unused)]
pub(super) type BucketBlobModel = <BucketBlobs as EntityTrait>::Model;
#[allow(unused)]
type BucketBlobActiveModel = entities::bucket_blobs::ActiveModel;
#[allow(unused)]
type BucketBlobColumn = <BucketBlobs as EntityTrait>::Column;

#[derive(Clone)]
#[allow(unused)]
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

#[allow(unused)]
impl BlobBucketDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
    ) -> JwstStorageResult<Self> {
        todo!()
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        todo!()
    }

    pub(super) async fn metadata(
        &self,
        workspace: &str,
        hash: &str,
    ) -> JwstBlobResult<InternalBlobMetadata> {
        BucketBlobs::find_by_id((workspace.into(), hash.into()))
            .select_only()
            .column_as(BucketBlobColumn::Length, "size")
            .column_as(BucketBlobColumn::Timestamp, "created_at")
            .into_model::<InternalBlobMetadata>()
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn exists(&self, workspace: &str, hash: &str) -> Result<bool, DbErr> {
        BucketBlobs::find_by_id((workspace.into(), hash.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)
    }

    pub(super) async fn get_blobs_size(&self, workspace: &str) -> Result<Option<i64>, DbErr> {
        BucketBlobs::find()
            .filter(BucketBlobColumn::Workspace.eq(workspace))
            .column_as(BucketBlobColumn::Length, "size")
            .column_as(BucketBlobColumn::Timestamp, "created_at")
            .into_model::<InternalBlobMetadata>()
            .all(&self.pool)
            .await
            .map(|r| r.into_iter().map(|f| f.size).reduce(|a, b| a + b))
    }

    async fn insert(&self, workspace: &str, hash: &str, blob: &[u8]) -> Result<(), DbErr> {
        if !self.exists(workspace, hash).await? {
            BucketBlobs::insert(BucketBlobActiveModel {
                workspace: Set(workspace.into()),
                hash: Set(hash.into()),
                length: Set(blob.len().try_into().unwrap()),
                timestamp: Set(Utc::now().into()),
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
            .filter(BucketBlobColumn::Workspace.eq(workspace))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct BucketStorage {
    pub(super) op: Operator,
}

// TODO Builder for BucketStorage;
// TODO add retry layer

#[allow(unused_variables)]
#[async_trait]
impl BucketBlobStorage<JwstStorageError> for BucketStorage {
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        todo!()
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        hash: String,
        blob: Vec<u8>,
    ) -> JwstResult<String, JwstStorageError> {
        todo!()
    }

    async fn delete_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        let workspace_id = workspace.unwrap_or("__default__".into());
        match self.op.delete(&format!("{}/{}", workspace_id, id)).await {
            Ok(_) => Ok(true),
            Err(e) => Err(JwstStorageError::from(e)),
        }
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        todo!()
    }
}

#[async_trait]
impl BlobStorage<JwstStorageError> for BlobBucketDBStorage {
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
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        self.bucket_storage.get_blob(workspace, id, params).await
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        _params: Option<HashMap<String, String>>,
    ) -> JwstResult<BlobMetadata, JwstStorageError> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(metadata) = self.metadata(&workspace, &id).await {
            Ok(metadata.into())
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, JwstStorageError> {
        let (hash, blob) = get_hash(stream).await;
        self.bucket_storage
            .put_blob(workspace.clone(), hash.clone(), blob.clone())
            .await?;
        let _lock = self.bucket.write().await;
        let workspace = workspace.unwrap_or("__default__".into());

        if self.insert(&workspace, &hash, &blob).await.is_ok() {
            Ok(hash)
        } else {
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
        let workspace = workspace.unwrap_or("__default__".into());
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
