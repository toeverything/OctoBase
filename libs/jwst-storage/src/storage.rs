use super::{blobs::BlobAutoStorage, docs::DocAutoStorage, *};
use jwst::info;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct JwstStorage {
    pool: DatabaseConnection,
    blobs: BlobAutoStorage,
    docs: DocAutoStorage,
}

impl JwstStorage {
    pub async fn new(database: &str) -> JwstResult<Self> {
        let pool = Database::connect(database)
            .await
            .context("Failed to connect to database")?;

        let blobs = BlobAutoStorage::init_with_pool(pool.clone())
            .await
            .context("Failed to init blobs")?;
        let docs = DocAutoStorage::init_with_pool(pool.clone())
            .await
            .context("Failed to init docs")?;

        Ok(Self { pool, blobs, docs })
    }

    pub async fn new_with_sqlite(file: &str) -> JwstResult<Self> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).context("Failed to create data directory")?;
        }

        Self::new(&format!(
            "sqlite:{}?mode=rwc",
            data.join(PathBuf::from(file).name_str())
                .with_extension("db")
                .display()
        ))
        .await
    }

    pub fn database(&self) -> String {
        format!("{:?}", self.pool)
    }

    pub fn blobs(&self) -> &BlobAutoStorage {
        &self.blobs
    }

    pub fn docs(&self) -> &DocAutoStorage {
        &self.docs
    }

    pub async fn with_pool<R, F, Fut>(&self, func: F) -> JwstResult<R>
    where
        F: Fn(DatabaseConnection) -> Fut,
        Fut: Future<Output = JwstResult<R>>,
    {
        func(self.pool.clone()).await
    }

    pub async fn create_workspace<S>(&self, workspace_id: S) -> JwstResult<Arc<RwLock<Workspace>>>
    where
        S: AsRef<str>,
    {
        info!("create_workspace: {}", workspace_id.as_ref());
        Ok(self
            .docs
            .get(workspace_id.as_ref().into())
            .await
            .context(format!(
                "Failed to create workspace {}",
                workspace_id.as_ref()
            ))?)
    }

    pub async fn get_workspace<S>(&self, workspace_id: S) -> JwstResult<Arc<RwLock<Workspace>>>
    where
        S: AsRef<str>,
    {
        info!("get_workspace: {}", workspace_id.as_ref());
        if self
            .docs
            .exists(workspace_id.as_ref().into())
            .await
            .context(format!(
                "Failed to check workspace {}",
                workspace_id.as_ref()
            ))?
        {
            Ok(self
                .docs
                .get(workspace_id.as_ref().into())
                .await
                .context(format!("Failed to get workspace {}", workspace_id.as_ref()))?)
        } else {
            Err(JwstError::WorkspaceNotFound(workspace_id.as_ref().into()))
        }
    }

    pub async fn full_migrate(&self, workspace_id: String, update: Option<Vec<u8>>) -> bool {
        if let Ok(workspace) = self.docs.get(workspace_id.clone()).await {
            let update = if let Some(update) = update {
                if let Err(e) = self.docs.delete(workspace_id.clone()).await {
                    error!("full_migrate write error: {}", e.to_string());
                    return false;
                };
                update
            } else {
                workspace.read().await.sync_migration()
            };
            if let Err(e) = self.docs.full_migrate(&workspace_id, update).await {
                error!("db write error: {}", e.to_string());
                return false;
            }
            return true;
        }
        false
    }
}
