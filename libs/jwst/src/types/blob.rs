use super::*;
use bytes::Bytes;
use chrono::NaiveDateTime;
use futures::Stream;
use std::collections::HashMap;

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
