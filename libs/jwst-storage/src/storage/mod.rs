mod blobs;
mod docs;
mod tests;

use super::*;
use blobs::BlobAutoStorage;
use docs::DocAutoStorage;
use std::{collections::HashMap, time::Instant};
use tokio::sync::Mutex;

pub struct JwstStorage {
    pool: DatabaseConnection,
    blobs: BlobAutoStorage,
    docs: DocAutoStorage,
    last_migrate: Mutex<HashMap<String, Instant>>,
}

impl JwstStorage {
    pub async fn new(database: &str) -> JwstResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;
        let bucket = get_bucket(is_sqlite);

        let blobs = BlobAutoStorage::init_with_pool(pool.clone(), bucket.clone())
            .await
            .context("Failed to init blobs")?;
        let docs = DocAutoStorage::init_with_pool(pool.clone(), bucket.clone())
            .await
            .context("Failed to init docs")?;

        Ok(Self {
            pool,
            blobs,
            docs,
            last_migrate: Mutex::new(HashMap::new()),
        })
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

    pub async fn create_workspace<S>(&self, workspace_id: S) -> JwstResult<Workspace>
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

    pub async fn get_workspace<S>(&self, workspace_id: S) -> JwstResult<Workspace>
    where
        S: AsRef<str>,
    {
        trace!("get_workspace: {}", workspace_id.as_ref());
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

    pub async fn full_migrate(
        &self,
        workspace_id: String,
        update: Option<Vec<u8>>,
        force: bool,
    ) -> bool {
        let mut map = self.last_migrate.lock().await;
        let ts = map.entry(workspace_id.clone()).or_insert(Instant::now());

        if ts.elapsed().as_secs() > 5 || force {
            info!("full migrate: {workspace_id}");
            if let Ok(workspace) = self.docs.get(workspace_id.clone()).await {
                let update = if let Some(update) = update {
                    if let Err(e) = self.docs.delete(workspace_id.clone()).await {
                        error!("full_migrate write error: {}", e.to_string());
                        return false;
                    };
                    update
                } else {
                    workspace.sync_migration()
                };
                if let Err(e) = self
                    .docs
                    .write_full_update(workspace_id.clone(), update)
                    .await
                {
                    error!("db write error: {}", e.to_string());
                    return false;
                }

                *ts = Instant::now();

                info!("full migrate final: {workspace_id}");
                return true;
            }
            warn!("workspace {workspace_id} not exists in cache");
            false
        } else {
            true
        }
    }
}
