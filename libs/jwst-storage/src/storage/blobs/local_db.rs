use super::{utils::get_hash, *};
use crate::types::JwstStorageResult;
use jwst::{Base64Engine, URL_SAFE_ENGINE};

use sha2::{Digest, Sha256};
pub(super) type BlobModel = <Blobs as EntityTrait>::Model;
type BlobActiveModel = super::entities::blobs::ActiveModel;
type BlobColumn = <Blobs as EntityTrait>::Column;

#[derive(Clone)]
pub struct BlobDBStorage {
    bucket: Arc<Bucket>,
    pub(super) pool: DatabaseConnection,
}

impl AsRef<DatabaseConnection> for BlobDBStorage {
    fn as_ref(&self) -> &DatabaseConnection {
        &self.pool
    }
}

impl BlobDBStorage {
    pub async fn init_with_pool(
        pool: DatabaseConnection,
        bucket: Arc<Bucket>,
    ) -> JwstStorageResult<Self> {
        Ok(Self { bucket, pool })
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        let is_sqlite = is_sqlite(database);
        let pool = create_connection(database, is_sqlite).await?;

        Self::init_with_pool(pool, get_bucket(is_sqlite)).await
    }

    #[allow(unused)]
    async fn all(&self, workspace: &str) -> Result<Vec<BlobModel>, DbErr> {
        Blobs::find()
            .filter(BlobColumn::WorkspaceId.eq(workspace))
            .all(&self.pool)
            .await
    }

    async fn keys(&self, workspace: &str) -> Result<Vec<String>, DbErr> {
        Blobs::find()
            .filter(BlobColumn::WorkspaceId.eq(workspace))
            .column(BlobColumn::Hash)
            .all(&self.pool)
            .await
            .map(|r| r.into_iter().map(|f| f.hash).collect())
    }

    #[allow(unused)]
    async fn count(&self, workspace: &str) -> Result<u64, DbErr> {
        Blobs::find()
            .filter(BlobColumn::WorkspaceId.eq(workspace))
            .count(&self.pool)
            .await
    }

    async fn exists(&self, workspace: &str, hash: &str) -> Result<bool, DbErr> {
        Blobs::find_by_id((workspace.into(), hash.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)
    }

    pub(super) async fn metadata(
        &self,
        workspace: &str,
        hash: &str,
    ) -> JwstBlobResult<InternalBlobMetadata> {
        Blobs::find_by_id((workspace.into(), hash.into()))
            .select_only()
            .column_as(BlobColumn::Length, "size")
            .column_as(BlobColumn::CreatedAt, "created_at")
            .into_model::<InternalBlobMetadata>()
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    pub(super) async fn get_blobs_size(&self, workspace: &str) -> Result<Option<i64>, DbErr> {
        Blobs::find()
            .filter(BlobColumn::WorkspaceId.eq(workspace))
            .column_as(BlobColumn::Length, "size")
            .column_as(BlobColumn::CreatedAt, "created_at")
            .into_model::<InternalBlobMetadata>()
            .all(&self.pool)
            .await
            .map(|r| r.into_iter().map(|f| f.size).reduce(|a, b| a + b))
    }

    async fn insert(&self, workspace: &str, hash: &str, blob: &[u8]) -> Result<(), DbErr> {
        if !self.exists(workspace, hash).await? {
            Blobs::insert(BlobActiveModel {
                workspace_id: Set(workspace.into()),
                hash: Set(hash.into()),
                blob: Set(blob.into()),
                length: Set(blob.len().try_into().unwrap()),
                created_at: Set(Utc::now().into()),
            })
            .exec(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub(super) async fn get(&self, workspace: &str, hash: &str) -> JwstBlobResult<BlobModel> {
        Blobs::find_by_id((workspace.into(), hash.into()))
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn delete(&self, workspace: &str, hash: &str) -> Result<bool, DbErr> {
        Blobs::delete_by_id((workspace.into(), hash.into()))
            .exec(&self.pool)
            .await
            .map(|r| r.rows_affected == 1)
    }

    async fn drop(&self, workspace: &str) -> Result<(), DbErr> {
        Blobs::delete_many()
            .filter(BlobColumn::WorkspaceId.eq(workspace))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl BlobStorage<JwstStorageError> for BlobDBStorage {
    async fn list_blobs(&self, workspace: Option<String>) -> JwstStorageResult<Vec<String>> {
        let _lock = self.bucket.read().await;
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(keys) = self.keys(&workspace).await {
            return Ok(keys);
        }

        Err(JwstStorageError::WorkspaceNotFound(workspace))
    }

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

    async fn put_blob_stream(
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

    async fn put_blob(
        &self,
        workspace: Option<String>,
        blob: Vec<u8>,
    ) -> JwstStorageResult<String> {
        let _lock = self.bucket.write().await;
        let workspace = workspace.unwrap_or("__default__".into());
        let mut hasher = Sha256::new();

        hasher.update(&blob);
        let hash = URL_SAFE_ENGINE.encode(hasher.finalize());

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
            workspace_id: "basic".into(),
            hash: "test".into(),
            blob: vec![1, 2, 3, 4],
            length: 4,
            created_at: all.get(0).unwrap().created_at
        }]
    );
    assert_eq!(pool.count("basic").await?, 1);
    assert_eq!(pool.keys("basic").await?, vec!["test"]);

    pool.drop("basic").await?;
    assert_eq!(pool.count("basic").await?, 0);
    assert_eq!(pool.keys("basic").await?, Vec::<String>::new());

    pool.insert("basic", "test1", &[1, 2, 3, 4]).await?;

    let all = pool.all("basic").await?;
    assert_eq!(
        all,
        vec![BlobModel {
            workspace_id: "basic".into(),
            hash: "test1".into(),
            blob: vec![1, 2, 3, 4],
            length: 4,
            created_at: all.get(0).unwrap().created_at
        }]
    );
    assert_eq!(pool.count("basic").await?, 1);
    assert_eq!(pool.keys("basic").await?, vec!["test1"]);

    let metadata = pool.metadata("basic", "test1").await?;

    assert_eq!(metadata.size, 4);
    assert!((metadata.created_at.timestamp() - Utc::now().timestamp()).abs() < 2);

    pool.drop("basic").await?;

    Ok(())
}
