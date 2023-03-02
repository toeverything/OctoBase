mod database;

use super::*;
use dashmap::DashMap;
use database::DocDBStorage;
use tokio::sync::broadcast::Sender;

#[cfg(test)]
pub(super) use database::docs_storage_test;
#[cfg(test)]
#[cfg(feature = "postgres")]
pub(super) use database::full_migration_test;

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

    pub fn remote(&self) -> &DashMap<String, Sender<Vec<u8>>> {
        self.0.remote()
    }
}

#[async_trait]
impl DocStorage for DocAutoStorage {
    async fn exists(&self, id: String) -> JwstResult<bool> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move { db.exists(id).await })
        })
        .await
        .context("failed to spawn query thread")?
    }

    async fn get(&self, id: String) -> JwstResult<Workspace> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move { db.get(id).await })
        })
        .await
        .context("failed to spawn query thread")?
    }

    async fn write_full_update(&self, id: String, data: Vec<u8>) -> JwstResult<()> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move { db.write_full_update(id, data).await })
        })
        .await
        .context("failed to spawn query thread")?
    }

    async fn write_update(&self, id: String, data: &[u8]) -> JwstResult<()> {
        let db = self.0.clone();
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move { db.write_update(id, &data).await })
        })
        .await
        .context("failed to spawn query thread")?
    }

    async fn delete(&self, id: String) -> JwstResult<()> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move { db.delete(id).await })
        })
        .await
        .context("failed to spawn query thread")?
    }
}

#[cfg(test)]
mod test {
    use super::{error, info, Arc, DocAutoStorage, DocStorage, Workspace};
    use dashmap::{mapref::entry::Entry, DashMap};
    use jwst::JwstError;
    use rand::random;
    use std::collections::HashSet;
    use threadpool::ThreadPool;
    use tokio::task::JoinSet;
    use yrs::Doc;

    #[ignore = "for stress testing"]
    #[test]
    fn dashmap_stress_test() -> anyhow::Result<()> {
        let workspaces: Arc<DashMap<String, Workspace>> = Arc::new(DashMap::new());
        let pool = ThreadPool::with_name("worker".into(), 10);
        let mut set = HashSet::new();

        for _ in 0..100000 {
            let id = random::<u64>().to_string();
            set.insert(id.clone());
            let workspaces = workspaces.clone();

            pool.execute(move || {
                let workspace = match workspaces.entry(id.clone()) {
                    Entry::Occupied(ws) => ws.get().clone(),
                    Entry::Vacant(v) => {
                        let ws = Workspace::from_doc(Doc::new(), id.clone());
                        v.insert(ws).clone()
                    }
                };
                assert_eq!(workspace.id(), id);
            });
        }

        pool.join();

        for id in set {
            let workspaces = workspaces.clone();
            pool.execute(move || {
                let workspace = workspaces.get(&id).unwrap();
                assert_eq!(workspace.id(), id);
            });
        }
        pool.join();

        Ok(())
    }

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
