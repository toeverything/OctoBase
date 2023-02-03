use super::Workspace;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::Stream;
use std::sync::Arc;
use tokio::{io, sync::RwLock};
use yrs::Doc;

#[async_trait]
pub trait DocStorage {
    async fn get(&self, workspace_id: String) -> io::Result<Arc<RwLock<Workspace>>>;
    async fn write_doc(&self, workspace_id: String, doc: &Doc) -> io::Result<()>;
    /// Return false means update exceeding max update
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> io::Result<bool>;
    async fn delete(&self, workspace_id: String) -> io::Result<()>;
}

#[async_trait]
pub trait DocSync {
    async fn sync(&self, id: String, remote: String) -> io::Result<()>;
}

#[derive(Debug)]
pub struct BlobMetadata {
    pub size: u64,
    pub last_modified: NaiveDateTime,
}

#[async_trait]
pub trait BlobStorage {
    type Read: Stream + Send;

    async fn get_blob(&self, workspace: Option<String>, id: String) -> io::Result<Self::Read>;
    async fn get_metadata(&self, workspace: Option<String>, id: String)
        -> io::Result<BlobMetadata>;
    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> io::Result<String>;
    async fn delete_blob(&self, workspace: Option<String>, id: String) -> io::Result<()>;
    async fn delete_workspace(&self, workspace_id: String) -> io::Result<()>;
}
