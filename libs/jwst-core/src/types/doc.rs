use yrs::Doc;

use super::*;

#[async_trait]
pub trait DocStorage<E = JwstError> {
    /// check if the workspace exists
    async fn detect_workspace(&self, workspace_id: &str) -> JwstResult<bool, E>;
    /// get a workspace or create a new one if not exists
    async fn get_or_create_workspace(&self, workspace_id: String) -> JwstResult<Workspace, E>;
    /// create a new workspace with given data or overwrite all update if exists
    async fn flush_workspace(
        &self,
        workspace_id: String,
        data: Vec<u8>,
    ) -> JwstResult<Workspace, E>;
    /// delete a workspace
    async fn delete_workspace(&self, workspace_id: &str) -> JwstResult<(), E>;

    /// check if the doc exists
    async fn detect_doc(&self, guid: &str) -> JwstResult<bool, E>;
    /// get a doc by it's guid
    async fn get_doc(&self, guid: String) -> JwstResult<Option<Doc>, E>;
    /// delete doc
    async fn delete_doc(&self, guid: &str) -> JwstResult<(), E>;
    /// integrate update into doc
    async fn update_doc(
        &self,
        workspace_id: String,
        guid: String,
        data: &[u8],
    ) -> JwstResult<(), E>;
    /// integrate update with subdoc information
    async fn update_doc_with_guid(&self, workspace_id: String, data: &[u8]) -> JwstResult<(), E>;
}
