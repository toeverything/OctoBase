use super::{utils::get_hash, *};
use crate::types::JwstStorageResult;
use jwst_storage_migration::{Migrator, MigratorTrait};

pub(super) type BlobModel = <Blobs as EntityTrait>::Model;
type BlobActiveModel = super::entities::blobs::ActiveModel;
type BlobColumn = <Blobs as EntityTrait>::Column;

#[derive(Clone)]
pub struct BlobDBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
}

impl BlobDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
    ) -> JwstStorageResult<Self> {
        Migrator::up(&pool, None).await?;
        Ok(Self { bucket, pool })
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite)).await
    }

    #[allow(unused)]
    async fn all(&self, table: &str) -> Result<Vec<BlobModel>, DbErr> {
        Blobs::find()
            .filter(BlobColumn::Workspace.eq(table))
            .all(&self.pool)
            .await
    }

    #[allow(unused)]
    async fn count(&self, table: &str) -> Result<u64, DbErr> {
        Blobs::find()
            .filter(BlobColumn::Workspace.eq(table))
            .count(&self.pool)
            .await
    }

    async fn exists(&self, table: &str, hash: &str) -> Result<bool, DbErr> {
        Blobs::find_by_id((table.into(), hash.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)
    }

    pub(super) async fn metadata(
        &self,
        table: &str,
        hash: &str,
    ) -> JwstBlobResult<InternalBlobMetadata> {
        Blobs::find_by_id((table.into(), hash.into()))
            .select_only()
            .column_as(BlobColumn::Length, "size")
            .column_as(BlobColumn::Timestamp, "created_at")
            .into_model::<InternalBlobMetadata>()
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    pub(super) async fn get_blobs_size(&self, workspace: &str) -> Result<Option<i64>, DbErr> {
        Blobs::find()
            .filter(BlobColumn::Workspace.eq(workspace))
            .column_as(BlobColumn::Length, "size")
            .column_as(BlobColumn::Timestamp, "created_at")
            .into_model::<InternalBlobMetadata>()
            .all(&self.pool)
            .await
            .map(|r| r.into_iter().map(|f| f.size).reduce(|a, b| a + b))
    }

    async fn insert(&self, table: &str, hash: &str, blob: &[u8]) -> Result<(), DbErr> {
        if !self.exists(table, hash).await? {
            Blobs::insert(BlobActiveModel {
                workspace: Set(table.into()),
                hash: Set(hash.into()),
                blob: Set(blob.into()),
                length: Set(blob.len().try_into().unwrap()),
                timestamp: Set(Utc::now().into()),
            })
            .exec(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub(super) async fn get(&self, table: &str, hash: &str) -> JwstBlobResult<BlobModel> {
        Blobs::find_by_id((table.into(), hash.into()))
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn delete(&self, table: &str, hash: &str) -> Result<bool, DbErr> {
        Blobs::delete_by_id((table.into(), hash.into()))
            .exec(&self.pool)
            .await
            .map(|r| r.rows_affected == 1)
    }

    async fn drop(&self, table: &str) -> Result<(), DbErr> {
        Blobs::delete_many()
            .filter(BlobColumn::Workspace.eq(table))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl BlobStorage<JwstStorageError> for BlobDBStorage {
    async fn check_blob(&self, workspace: Option<String>, id: String) -> JwstStorageResult<bool> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(exists) = self.exists(&workspace, &id).await {
            return Ok(exists);
        }

        Err(JwstStorageError::WorkspaceNotFound(workspace))
    }

    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        _params: Option<HashMap<String, String>>,
    ) -> JwstStorageResult<Vec<u8>> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(blob) = self.get(&workspace, &id).await {
            return Ok(blob.blob);
        }

        Err(JwstStorageError::WorkspaceNotFound(workspace))
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        _params: Option<HashMap<String, String>>,
    ) -> JwstStorageResult<BlobMetadata> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(metadata) = self.metadata(&workspace, &id).await {
            Ok(metadata.into())
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstStorageResult<String> {
        let _lock = self.bucket.write().await;
        let workspace = workspace.unwrap_or("__default__".into());

        let (hash, blob) = get_hash(stream).await;

        if self.insert(&workspace, &hash, &blob).await.is_ok() {
            Ok(hash)
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace))
        }
    }

    async fn delete_blob(
        &self,
        workspace_id: Option<String>,
        id: String,
    ) -> JwstStorageResult<bool> {
        let _lock = self.bucket.write().await;
        let workspace_id = workspace_id.unwrap_or("__default__".into());
        if let Ok(success) = self.delete(&workspace_id, &id).await {
            Ok(success)
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace_id))
        }
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstStorageResult<()> {
        let _lock = self.bucket.write().await;
        if self.drop(&workspace_id).await.is_ok() {
            Ok(())
        } else {
            Err(JwstStorageError::WorkspaceNotFound(workspace_id))
        }
    }

    async fn get_blobs_size(&self, workspace_id: String) -> JwstStorageResult<i64> {
        let _lock = self.bucket.read().await;
        let size = self.get_blobs_size(&workspace_id).await?;
        return Ok(size.unwrap_or(0));
    }
}

#[cfg(test)]
pub async fn blobs_storage_test(pool: &BlobDBStorage) -> anyhow::Result<()> {
    // empty table
    assert_eq!(pool.count("basic").await?, 0);

    // first insert
    pool.insert("basic", "test", &[1, 2, 3, 4]).await?;
    assert_eq!(pool.count("basic").await?, 1);

    let all = pool.all("basic").await?;
    assert_eq!(
        all,
        vec![BlobModel {
            workspace: "basic".into(),
            hash: "test".into(),
            blob: vec![1, 2, 3, 4],
            length: 4,
            timestamp: all.get(0).unwrap().timestamp
        }]
    );
    assert_eq!(pool.count("basic").await?, 1);

    pool.drop("basic").await?;
    assert_eq!(pool.count("basic").await?, 0);

    pool.insert("basic", "test1", &[1, 2, 3, 4]).await?;

    let all = pool.all("basic").await?;
    assert_eq!(
        all,
        vec![BlobModel {
            workspace: "basic".into(),
            hash: "test1".into(),
            blob: vec![1, 2, 3, 4],
            length: 4,
            timestamp: all.get(0).unwrap().timestamp
        }]
    );
    assert_eq!(pool.count("basic").await?, 1);

    let metadata = pool.metadata("basic", "test1").await?;

    assert_eq!(metadata.size, 4);
    assert!((metadata.created_at.timestamp() - Utc::now().timestamp()).abs() < 2);

    pool.drop("basic").await?;

    Ok(())
}
