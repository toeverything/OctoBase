use std::collections::HashMap;

use chrono::NaiveDateTime;

use super::*;

#[derive(Debug)]
pub struct BlobMetadata {
    pub content_type: String,
    pub last_modified: NaiveDateTime,
    pub size: i64,
}

#[async_trait]
pub trait BlobStorage<E = JwstError> {
    async fn list_blobs(&self, workspace: Option<String>) -> JwstResult<Vec<String>, E>;
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
    async fn put_blob(&self, workspace: Option<String>, id: String, blob: Vec<u8>) -> JwstResult<String, E>;
    async fn delete_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool, E>;
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), E>;
    async fn get_blobs_size(&self, workspaces: Vec<String>) -> JwstResult<i64, E>;
}

#[async_trait]
pub trait BucketBlobStorage<E = JwstError> {
    async fn get_blob(&self, workspace: Option<String>, id: String) -> JwstResult<Vec<u8>, E>;
    async fn put_blob(&self, workspace: Option<String>, hash: String, blob: Vec<u8>) -> JwstResult<(), E>;
    async fn delete_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool, E>;
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<(), E>;
}
