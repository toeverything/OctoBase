mod database;

use super::*;
use database::DocDBStorage;
use tokio::sync::{broadcast::Sender, RwLock};

#[cfg(test)]
#[cfg(feature = "postgres")]
pub(super) use database::full_migration_stress_test;
#[cfg(test)]
pub(super) use database::{docs_storage_partial_test, docs_storage_test};

#[derive(Clone)]
pub struct DocAutoStorage(pub(super) Arc<DocDBStorage>);

impl DocAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection, bucket: Arc<Bucket>) -> JwstResult<Self> {
        Ok(Self(Arc::new(
            DocDBStorage::init_with_pool(pool, bucket).await?,
        )))
    }

    pub async fn init_pool(database: &str) -> JwstResult<Self> {
        Ok(Self(Arc::new(DocDBStorage::init_pool(database).await?)))
    }

    pub fn remote(&self) -> &RwLock<HashMap<String, Sender<Vec<u8>>>> {
        self.0.remote()
    }
}

#[async_trait]
impl DocStorage for DocAutoStorage {
    async fn exists(&self, id: String) -> JwstResult<bool> {
        self.0.exists(id).await
    }

    async fn get(&self, id: String) -> JwstResult<Workspace> {
        self.0.get(id).await
    }

    async fn write_full_update(&self, id: String, data: Vec<u8>) -> JwstResult<()> {
        self.0.write_full_update(id, data).await
    }

    async fn write_update(&self, id: String, data: &[u8]) -> JwstResult<()> {
        self.0.write_update(id, data).await
    }

    async fn delete(&self, id: String) -> JwstResult<()> {
        self.0.delete(id).await
    }
}

#[cfg(test)]
mod test {
    use super::{error, info, DocAutoStorage, DocStorage};
    use jwst::JwstError;
    use rand::random;
    use std::collections::HashSet;
    use tokio::task::JoinSet;

    async fn create_workspace_stress_test(storage: DocAutoStorage) -> anyhow::Result<()> {
        let mut join_set = JoinSet::new();
        let mut set = HashSet::new();

        for _ in 0..10000 {
            let id = random::<u64>().to_string();
            set.insert(id.clone());
            let storage = storage.clone();

            join_set.spawn(async move {
                info!("create workspace: {}", id);
                let workspace = storage.get(id.clone()).await?;
                info!("create workspace finish: {}", id);
                assert_eq!(workspace.id(), id);
                Ok::<_, JwstError>(())
            });
        }

        let mut a = 0;
        while let Some(ret) = join_set.join_next().await {
            if let Err(e) = ret? {
                error!("failed to execute creator: {e}");
            }
            a += 1;
            info!("{a}");
        }

        for (i, id) in set.iter().enumerate() {
            let storage = storage.clone();
            info!("check {i}: {id}");
            let id = id.clone();
            tokio::spawn(async move {
                info!("get workspace: {}", id);
                let workspace = storage.get(id.clone()).await?;
                assert_eq!(workspace.id(), id);
                Ok::<_, JwstError>(())
            });
        }

        while let Some(ret) = join_set.join_next().await {
            if let Err(e) = ret? {
                error!("failed to execute: {e}");
            }
        }

        Ok(())
    }

    #[ignore = "for stress testing"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn sqlite_create_workspace_stress_test() -> anyhow::Result<()> {
        jwst_logger::init_logger();
        let storage = DocAutoStorage::init_pool("sqlite::memory:").await?;
        create_workspace_stress_test(storage.clone()).await?;

        Ok(())
    }

    #[ignore = "for stress testing"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn postgres_create_workspace_stress_test() -> anyhow::Result<()> {
        jwst_logger::init_logger();
        let storage =
            DocAutoStorage::init_pool("postgresql://affine:affine@localhost:5432/affine_binary")
                .await?;
        create_workspace_stress_test(storage.clone()).await?;

        Ok(())
    }

    #[ignore = "for stress testing"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn mysql_create_workspace_stress_test() -> anyhow::Result<()> {
        // jwst_logger::init_logger();
        let storage =
            DocAutoStorage::init_pool("mysql://affine:affine@localhost:3306/affine_binary").await?;
        create_workspace_stress_test(storage.clone()).await?;

        Ok(())
    }
}
