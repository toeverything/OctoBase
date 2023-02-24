use super::Workspace;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::Stream;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum JwstError {
    // #[error("database error")]
    // Database(#[from] DbErr),
    #[error(transparent)]
    BoxedError(#[from] anyhow::Error),
    #[error(transparent)]
    StorageError(anyhow::Error),
    #[error("workspace {0} not initialized")]
    WorkspaceNotInitialized(String),
    #[error("workspace {0} not found")]
    WorkspaceNotFound(String),
}

pub type JwstResult<T> = Result<T, JwstError>;

#[async_trait]
pub trait DocStorage {
    async fn exists(&self, workspace_id: String) -> JwstResult<bool>;
    async fn get(&self, workspace_id: String) -> JwstResult<Arc<RwLock<Workspace>>>;
    async fn write_full_update(&self, workspace_id: String, data: Vec<u8>) -> JwstResult<()>;
    /// Return false means update exceeding max update
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<()>;
    async fn delete(&self, workspace_id: String) -> JwstResult<()>;
}

#[async_trait]
pub trait DocSync {
    async fn sync(&self, id: String, remote: String) -> JwstResult<Arc<RwLock<Workspace>>>;
}

#[derive(Debug)]
pub struct BlobMetadata {
    pub size: u64,
    pub last_modified: NaiveDateTime,
}

#[async_trait]
pub trait BlobStorage {
    type Read: Stream + Send;

    async fn get_blob(&self, workspace: Option<String>, id: String) -> JwstResult<Self::Read>;
    async fn get_metadata(&self, workspace: Option<String>, id: String)
        -> JwstResult<BlobMetadata>;
    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String>;
    async fn delete_blob(&self, workspace: Option<String>, id: String) -> JwstResult<()>;
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<()>;
}
