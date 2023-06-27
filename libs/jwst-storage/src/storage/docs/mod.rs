mod database;
mod utils;

use std::ops::Deref;

use super::*;
use database::DocDBStorage;
use tokio::sync::{broadcast::Sender, RwLock};

#[cfg(test)]
#[cfg(feature = "postgres")]
pub(super) use database::full_migration_stress_test;
#[cfg(test)]
pub(super) use database::{docs_storage_partial_test, docs_storage_test};

#[derive(Clone)]
pub struct SharedDocDBStorage(pub(super) Arc<DocDBStorage>);

impl SharedDocDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
    ) -> JwstStorageResult<Self> {
        Ok(Self(Arc::new(
            DocDBStorage::init_with_pool(pool, bucket).await?,
        )))
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        Ok(Self(Arc::new(DocDBStorage::init_pool(database).await?)))
    }

    pub fn remote(&self) -> &RwLock<HashMap<String, Sender<Vec<u8>>>> {
        self.0.remote()
    }
}

impl Deref for SharedDocDBStorage {
    type Target = DocDBStorage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::{error, info, DocStorage, SharedDocDBStorage};
    use crate::{JwstStorageError, JwstStorageResult};
    use jwst_storage_migration::Migrator;
    use rand::random;
    use sea_orm_migration::MigratorTrait;
    use std::collections::HashSet;
    use tokio::task::JoinSet;

    async fn create_workspace_stress_test(
        storage: SharedDocDBStorage,
        range: usize,
    ) -> JwstStorageResult<()> {
        let mut join_set = JoinSet::new();
        let mut set = HashSet::new();

        for _ in 0..range {
            let id = random::<u64>().to_string();
            set.insert(id.clone());
            let storage = storage.clone();

            join_set.spawn(async move {
                info!("create workspace: {}", id);
                let workspace = storage.get_or_create_workspace(id.clone()).await?;
                info!("create workspace finish: {}", id);
                assert_eq!(workspace.id(), id);
                Ok::<_, JwstStorageError>(())
            });
        }

        while let Some(ret) = join_set.join_next().await {
            if let Err(e) = ret.map_err(JwstStorageError::DocMerge)? {
                error!("failed to execute creator: {e}");
            }
        }

        for (i, id) in set.iter().enumerate() {
            let storage = storage.clone();
            info!("check {i}: {id}");
            let id = id.clone();
            tokio::spawn(async move {
                info!("get workspace: {}", id);
                let workspace = storage.get_or_create_workspace(id.clone()).await?;
                assert_eq!(workspace.id(), id);
                Ok::<_, JwstStorageError>(())
            });
        }

        while let Some(ret) = join_set.join_next().await {
            if let Err(e) = ret.map_err(JwstStorageError::DocMerge)? {
                error!("failed to execute: {e}");
            }
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sqlite_create_workspace_stress_test_faster() -> anyhow::Result<()> {
        jwst_logger::init_logger("jwst-storage");
        let storage = SharedDocDBStorage::init_pool("sqlite::memory:").await?;
        Migrator::up(&storage.0.pool, None).await.unwrap();
        create_workspace_stress_test(storage.clone(), 100).await?;

        Ok(())
    }

    #[ignore = "for stress testing"]
    #[tokio::test(flavor = "multi_thread")]
    async fn sqlite_create_workspace_stress_test() -> anyhow::Result<()> {
        jwst_logger::init_logger("jwst-storage");
        let storage = SharedDocDBStorage::init_pool("sqlite::memory:").await?;
        create_workspace_stress_test(storage.clone(), 10000).await?;

        Ok(())
    }

    #[ignore = "for stress testing"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn postgres_create_workspace_stress_test() -> anyhow::Result<()> {
        jwst_logger::init_logger("jwst-storage");
        let storage = SharedDocDBStorage::init_pool(
            "postgresql://affine:affine@localhost:5432/affine_binary",
        )
        .await?;
        create_workspace_stress_test(storage.clone(), 10000).await?;

        Ok(())
    }

    #[ignore = "for stress testing"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn mysql_create_workspace_stress_test() -> anyhow::Result<()> {
        // jwst_logger::init_logger();
        let storage =
            SharedDocDBStorage::init_pool("mysql://affine:affine@localhost:3306/affine_binary")
                .await?;
        create_workspace_stress_test(storage.clone(), 10000).await?;

        Ok(())
    }
}
