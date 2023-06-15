use super::*;
use crate::entities::prelude::S3Blobs;
use crate::rate_limiter::Bucket;
use crate::JwstStorageError;
use bytes::Bytes;
use futures::Stream;
use jwst::{BlobMetadata, BlobStorage, JwstResult};
use opendal::Operator;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::collections::HashMap;
use std::sync::Arc;

#[allow(unused)]
pub(super) type S3BlobModel = <S3Blobs as EntityTrait>::Model;
#[allow(unused)]
type BlobActiveModel = entities::s3_blobs::ActiveModel;
#[allow(unused)]
type BlobColumn = <S3Blobs as EntityTrait>::Column;

#[derive(Clone)]
#[allow(unused)]
pub struct BlobS3DBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
    pub(super) s3_storage: S3Storage,
}

impl AsRef<DatabaseConnection> for BlobS3DBStorage {
    fn as_ref(&self) -> &DatabaseConnection {
        &self.pool
    }
}

#[allow(unused)]
impl BlobS3DBStorage {
    pub async fn init_with_pool() -> JwstStorageResult<Self> {
        todo!()
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        todo!()
    }
}

#[derive(Clone)]
pub struct S3Storage {
    pub(super) op: Operator,
}

// TODO Builder for S3Storage;
// TODO add retry layer

#[allow(unused_variables)]
#[async_trait]
impl BlobStorage<JwstStorageError> for S3Storage {
    async fn check_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        todo!()
    }

    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        todo!()
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<BlobMetadata, JwstStorageError> {
        todo!()
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
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

    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, JwstStorageError> {
        todo!()
    }
}

#[allow(unused_variables)]
#[async_trait]
impl BlobStorage<JwstStorageError> for BlobS3DBStorage {
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
        self.s3_storage.get_blob(workspace, id, params).await
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
    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, JwstStorageError> {
        self.s3_storage
            .put_blob(workspace.clone(), stream).await?;
        todo!()
    }

    // db and s3 operation
    async fn delete_blob(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<bool, JwstStorageError> {
        self.s3_storage.delete_blob(workspace.clone(), id.clone()).await?;
        todo!()
    }

    // db and s3 operation
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        self.s3_storage.delete_workspace(workspace_id.clone()).await?;
        todo!()
    }

    // only db operation
    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, JwstStorageError> {
        todo!()
    }
}
