use super::*;

#[async_trait]
pub trait DocStorage<E = JwstError> {
    async fn exists(&self, workspace_id: String) -> JwstResult<bool, E>;
    async fn get(&self, workspace_id: String) -> JwstResult<Workspace, E>;
    async fn write_full_update(&self, workspace_id: String, data: Vec<u8>) -> JwstResult<(), E>;
    /// Return false means update exceeding max update
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> JwstResult<(), E>;
    async fn delete(&self, workspace_id: String) -> JwstResult<(), E>;
}
