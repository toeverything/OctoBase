use super::Workspace;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::Stream;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwstError {
    // #[error("database error")]
    // Database(#[from] DbErr),
    #[error(transparent)]
    BoxedError(#[from] anyhow::Error),
    #[error(transparent)]
    StorageError(anyhow::Error),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("doc codec error")]
    DocCodec(#[from] lib0::error::Error),
    #[error("doc transaction error")]
    DocTransaction(String),
    #[error("workspace {0} not initialized")]
    WorkspaceNotInitialized(String),
    // version metadata
    #[error("workspace {0} has no version")]
    VersionNotFound(String),
    // page metadata
    #[error("workspace {0} has no page tree")]
    PageTreeNotFound(String),
    #[error("page item {0} not found")]
    PageItemNotFound(String),
    #[error("failed to get state vector")]
    SyncInitTransaction,
    #[error("y_sync awareness error")]
    YSyncAwarenessErr(#[from] y_sync::awareness::Error),
}

pub type JwstResult<T, E = JwstError> = Result<T, E>;

#[async_trait]
pub trait DocStorage<E = JwstError> {
    async fn exists(&self, workspace_id: String) -> JwstResult<bool, E>;
    async fn get(&self, workspace_id: String) -> JwstResult<Workspace, E>;
    async fn write_full_update(&self, workspace_id: String, data: Vec<u8>) -> JwstResult<(), E>;
    /// Return false means update exceeding max update
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<(), E>;
    async fn delete(&self, workspace_id: String) -> JwstResult<(), E>;
}

#[derive(Debug)]
pub struct BlobMetadata {
    pub content_type: String,
    pub last_modified: NaiveDateTime,
    pub size: u64,
}

#[async_trait]
pub trait BlobStorage<E = JwstError> {
    async fn check_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool, E>;
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Vec<u8>, E>;
    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<BlobMetadata, E>;
    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String, E>;
    async fn delete_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool, E>;
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), E>;
    async fn get_blobs_size(&self, workspace_id: String) -> JwstResult<i64, E>;
}
