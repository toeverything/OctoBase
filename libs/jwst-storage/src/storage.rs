use super::{blobs::BlobAutoStorage, docs::DocAutoStorage, *};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct JwstStorage {
    pool: DatabaseConnection,
    blobs: BlobAutoStorage,
    docs: DocAutoStorage,
}

impl JwstStorage {
    pub async fn new(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;

        let blobs = BlobAutoStorage::init_with_pool(pool.clone()).await?;
        let docs = DocAutoStorage::init_with_pool(pool.clone()).await?;

        Ok(Self { pool, blobs, docs })
    }

    pub async fn new_with_sqlite(file: &str) -> Result<Self, DbErr> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).map_err(|e| DbErr::Custom(e.to_string()))?;
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

    pub async fn with_pool<R, F, Fut>(&self, func: F) -> Result<R, DbErr>
    where
        F: Fn(DatabaseConnection) -> Fut,
        Fut: Future<Output = Result<R, DbErr>>,
    {
        func(self.pool.clone()).await
    }

    pub async fn get_workspace(&self, workspace_id: String) -> Arc<RwLock<Workspace>> {
        self.docs
            .get(workspace_id.clone())
            .await
            .expect("Failed to get workspace")
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
