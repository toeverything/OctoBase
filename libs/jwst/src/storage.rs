use async_trait::async_trait;
use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::Stream;
use std::path::Path;
use tokio::io;
use yrs::Doc;

#[async_trait]
pub trait DocStorage {
    async fn get(&self, workspace_id: i64) -> io::Result<Doc>;
    async fn write_doc(&self, workspace_id: i64, doc: &Doc) -> io::Result<()>;
    /// Return false means update exceeding max update
    async fn write_update(&self, workspace: i64, data: &[u8]) -> io::Result<bool>;
    async fn delete(&self, workspace_id: i64) -> io::Result<()>;
}

pub struct BlobMetadata {
    pub size: u64,
    pub last_modified: NaiveDateTime,
}

#[async_trait]
pub trait BlobStorage {
    type Read: Stream + Send;

    async fn get(&self, path: impl AsRef<Path> + Send) -> io::Result<Self::Read>;
    async fn get_metedata(&self, path: impl AsRef<Path> + Send) -> io::Result<BlobMetadata>;
    async fn put(
        &self,
        stream: impl Stream<Item = Bytes> + Send,
        prefix: Option<String>,
    ) -> io::Result<String>;
    async fn rename(
        &self,
        from: impl AsRef<Path> + Send,
        to: impl AsRef<Path> + Send,
    ) -> io::Result<()>;
    async fn delete(&self, path: impl AsRef<Path> + Send) -> io::Result<()>;
}
