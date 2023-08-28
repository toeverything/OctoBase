#[cfg(feature = "bucket")]
mod bucket;
#[cfg(feature = "bucket")]
pub use bucket::{BlobBucketStorage, MixedBucketDBParam};

#[cfg(feature = "image")]
mod auto_storage;
#[cfg(feature = "image")]
pub use auto_storage::{BlobAutoStorage, ImageError, ImageParams};

mod blob_storage;
mod utils;

#[cfg(test)]
pub use blob_storage::blobs_storage_test;
pub use blob_storage::BlobDBStorage;
use bytes::Bytes;
use jwst_core::{BlobMetadata, BlobStorage};
use thiserror::Error;
use tokio::task::JoinError;
use utils::{get_hash, InternalBlobMetadata};

use super::{entities::prelude::*, *};

#[derive(Debug, Error)]
pub enum JwstBlobError {
    #[error("blob not found: {0}")]
    BlobNotFound(String),
    #[error("database error")]
    Database(#[from] DbErr),
    #[cfg(feature = "image")]
    #[error("failed to optimize image")]
    Image(#[from] ImageError),
    #[error("failed to optimize image")]
    ImageThread(#[from] JoinError),
    #[error("optimize params error: {0:?}")]
    ImageParams(HashMap<String, String>),
}
pub type JwstBlobResult<T> = Result<T, JwstBlobError>;

pub enum JwstBlobStorage {
    RawStorage(Arc<BlobDBStorage>),
    #[cfg(feature = "image")]
    AutoStorage(BlobAutoStorage),
    #[cfg(feature = "bucket")]
    BucketStorage(BlobBucketStorage),
}

pub enum BlobStorageType {
    DB,
    #[cfg(feature = "bucket")]
    MixedBucketDB(MixedBucketDBParam),
}

#[async_trait]
impl BlobStorage<JwstStorageError> for JwstBlobStorage {
    async fn list_blobs(&self, workspace: Option<String>) -> JwstResult<Vec<String>, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.list_blobs(workspace).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.list_blobs(workspace).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.list_blobs(workspace).await,
        }
    }

    async fn check_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.check_blob(workspace, id).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.check_blob(workspace, id).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.check_blob(workspace, id).await,
        }
    }

    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.get_blob(workspace, id, params).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.get_blob(workspace, id, params).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.get_blob(workspace, id, params).await,
        }
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<BlobMetadata, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.get_metadata(workspace, id, params).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.get_metadata(workspace, id, params).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.get_metadata(workspace, id, params).await,
        }
    }

    async fn put_blob_stream(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.put_blob_stream(workspace, stream).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.put_blob_stream(workspace, stream).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.put_blob_stream(workspace, stream).await,
        }
    }

    async fn put_blob(&self, workspace: Option<String>, blob: Vec<u8>) -> JwstResult<String, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.put_blob(workspace, blob).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.put_blob(workspace, blob).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.put_blob(workspace, blob).await,
        }
    }

    async fn delete_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.delete_blob(workspace, id).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.delete_blob(workspace, id).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.delete_blob(workspace, id).await,
        }
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => db.delete_workspace(workspace_id).await,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.delete_workspace(workspace_id).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.delete_workspace(workspace_id).await,
        }
    }

    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, JwstStorageError> {
        match self {
            JwstBlobStorage::RawStorage(db) => Ok(db.get_blobs_size(&workspace_id).await?.unwrap_or(0)),
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => db.get_blobs_size(workspace_id).await,
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(db) => db.get_blobs_size(workspace_id).await,
        }
    }
}

impl JwstBlobStorage {
    pub fn get_blob_db(&self) -> Option<Arc<BlobDBStorage>> {
        match self {
            JwstBlobStorage::RawStorage(db) => Some(db.clone()),
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(db) => Some(db.db.clone()),
            #[cfg(feature = "bucket")]
            JwstBlobStorage::BucketStorage(_) => None,
        }
    }

    #[cfg(feature = "bucket")]
    pub fn get_mixed_bucket_db(&self) -> Option<BlobBucketStorage> {
        match self {
            JwstBlobStorage::RawStorage(_) => None,
            #[cfg(feature = "image")]
            JwstBlobStorage::AutoStorage(_) => None,
            JwstBlobStorage::BucketStorage(db) => Some(db.clone()),
        }
    }
}
