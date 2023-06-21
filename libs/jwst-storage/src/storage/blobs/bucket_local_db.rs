use super::utils::calculate_hash;
use super::utils::get_hash;
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
#[async_trait]
impl BucketBlobStorage<JwstStorageError> for BucketStorage {
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        // TODO: params is not used for now.
        _params: Option<HashMap<String, String>>,
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
    ) -> JwstResult<String, JwstStorageError> {
        let workspace = get_workspace(workspace);
        let key = build_key(workspace, hash);
        let _ = self.op.write(&key, blob).await?;

        // FIXME: what's the meaning of string?
        Ok("".to_string())
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

#[allow(unused_variables)]
#[async_trait]
impl BlobStorage<JwstStorageError> for BlobBucketDBStorage {
    // only db operation
    async fn check_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        let workspace = get_workspace(workspace);
        let key = build_key(workspace, id);

        self.bucket_storage
            .op
            .is_exist(&key)
            .await
            .map_err(JwstStorageError::from)
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
        let workspace = get_workspace(workspace);
        let key = build_key(workspace, id);

        let metadata = self.bucket_storage.op.stat(&key).await?;

        Ok(BlobMetadata {
            content_type: metadata
                .content_type()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            last_modified: metadata
                .last_modified()
                .map(|v| v.naive_utc())
                .unwrap_or_default(),
            size: metadata.content_length(),
        })
    }

    // db and s3 operation
    async fn put_blob_stream(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, JwstStorageError> {
        let (hash, bs) = get_hash(stream).await;

        self.bucket_storage.put_blob(workspace, hash, bs).await
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        blob: Vec<u8>,
    ) -> JwstResult<String, JwstStorageError> {
        let hash = calculate_hash(&blob);

        self.bucket_storage.put_blob(workspace, hash, blob).await
    }

    // db and s3 operation
    async fn delete_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        self.bucket_storage.delete_blob(workspace, id).await
    }

    // db and s3 operation
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        self.bucket_storage.delete_workspace(workspace_id).await
    }

    // only db operation
    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, JwstStorageError> {
        todo!()
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
