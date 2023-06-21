use super::*;
use crate::rate_limiter::Bucket;
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
    pub async fn init_with_pool() -> JwstStorageResult<Self> {
        todo!()
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        todo!()
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

#[allow(unused_variables)]
#[async_trait]
impl BlobStorage<JwstStorageError> for BlobBucketDBStorage {
    // only db operation
    async fn check_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        todo!()
    }

    // only s3_storage operation
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        self.bucket_storage.get_blob(workspace, id, params).await
    }

    // only db operation
    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<BlobMetadata, JwstStorageError> {
        todo!()
    }

    // db and s3 operation
    async fn put_blob_stream(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, JwstStorageError> {
        todo!()
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        blob: Vec<u8>,
    ) -> JwstResult<String, JwstStorageError> {
        todo!()
    }

    // db and s3 operation
    async fn delete_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        self.bucket_storage
            .delete_blob(workspace.clone(), id.clone())
            .await?;
        todo!()
    }

    // db and s3 operation
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        self.bucket_storage
            .delete_workspace(workspace_id.clone())
            .await?;
        todo!()
    }

    // only db operation
    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, JwstStorageError> {
        todo!()
    }
}
