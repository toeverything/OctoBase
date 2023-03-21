mod blobs;
mod utils;

#[cfg(test)]
pub use blobs::blobs_storage_test;

use super::{entities::prelude::*, *};
use blobs::BlobDBStorage;
use bytes::Bytes;
use image::ImageError;
use jwst::{BlobMetadata, BlobStorage};
use thiserror::Error;
use tokio::task::JoinError;
use tokio_util::io::ReaderStream;
use utils::ImageParams;

#[derive(Debug, Error)]
pub enum JwstBlobError {
    #[error("failed to optimize image")]
    Image(#[from] ImageError),
    #[error("failed to optimize image")]
    ImageThread(#[from] JoinError),
    #[error("database error")]
    Database(#[from] DbErr),
    #[error("blob not found: {0}")]
    BlobNotFound(String),
    #[error("params error: {0:?}")]
    Params(HashMap<String, String>),
}
pub type JwstBlobResult<T> = Result<T, JwstBlobError>;

pub(super) type OptimizedBlobModel = <OptimizedBlobs as EntityTrait>::Model;
type OptimizedBlobActiveModel = super::entities::optimized_blobs::ActiveModel;
type OptimizedBlobColumn = <OptimizedBlobs as EntityTrait>::Column;

#[derive(Clone)]
pub struct BlobAutoStorage {
    pub(super) db: Arc<BlobDBStorage>,
    pool: DatabaseConnection,
}

impl BlobAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection, bucket: Arc<Bucket>) -> JwstResult<Self> {
        let db = Arc::new(BlobDBStorage::init_with_pool(pool, bucket).await?);
        let pool = db.pool.clone();
        Ok(Self { db, pool })
    }

    pub async fn init_pool(database: &str) -> JwstResult<Self> {
        let db = Arc::new(BlobDBStorage::init_pool(database).await?);
        let pool = db.pool.clone();
        Ok(Self { db, pool })
    }

    async fn exists(&self, table: &str, hash: &str) -> JwstBlobResult<bool> {
        Ok(Blobs::find_by_id((table.into(), hash.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)?)
    }

    async fn insert(
        &self,
        table: &str,
        hash: &str,
        params: &str,
        blob: &[u8],
    ) -> JwstBlobResult<()> {
        if !self.exists(table, hash).await? {
            OptimizedBlobs::insert(OptimizedBlobActiveModel {
                workspace: Set(table.into()),
                hash: Set(hash.into()),
                blob: Set(blob.into()),
                length: Set(blob.len().try_into().unwrap()),
                timestamp: Set(Utc::now().into()),
                params: Set(params.into()),
            })
            .exec(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn get(
        &self,
        table: &str,
        hash: &str,
        params: &str,
    ) -> JwstBlobResult<OptimizedBlobModel> {
        OptimizedBlobs::find_by_id((table.into(), hash.into(), params.into()))
            .one(&self.pool)
            .await
            .map_err(JwstBlobError::Database)
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn get_auto(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstBlobResult<Vec<u8>> {
        let workspace_id = workspace.as_deref().unwrap_or("__default__");
        if let Some(params) = params {
            if let Ok(params) = ImageParams::try_from(&params) {
                let params_token = params.to_string();
                if let Ok(blob) = self.get(workspace_id, &id, &params_token).await {
                    info!(
                        "exists optimized image: {} {} {}, {}bytes",
                        workspace_id,
                        id,
                        params_token,
                        blob.blob.len()
                    );
                    Ok(blob.blob)
                } else {
                    // TODO: need ddos mitigation
                    let blob = self.db.get(workspace_id, &id).await?;
                    let blob_len = blob.blob.len();
                    let image =
                        tokio::task::spawn_blocking(move || params.optimize_image(&blob.blob))
                            .await??;
                    self.insert(&workspace_id, &id, &params_token, &image)
                        .await?;
                    info!(
                        "optimized image: {} {} {}, {}bytes -> {}bytes",
                        workspace_id,
                        id,
                        params_token,
                        blob_len,
                        image.len()
                    );
                    Ok(image)
                }
            } else {
                Err(JwstBlobError::Params(params))
            }
        } else {
            self.db.get(workspace_id, &id).await.map(|m| m.blob)
        }
    }

    async fn delete(&self, table: &str, hash: &str) -> JwstBlobResult<bool> {
        Ok(Blobs::delete_by_id((table.into(), hash.into()))
            .exec(&self.pool)
            .await
            .map(|r| r.rows_affected == 1)?)
    }

    async fn drop(&self, table: &str) -> Result<(), DbErr> {
        Blobs::delete_many()
            .filter(OptimizedBlobColumn::Workspace.eq(table))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl BlobStorage for BlobAutoStorage {
    type Read = ReaderStream<Cursor<Vec<u8>>>;

    async fn check_blob(&self, workspace: Option<String>, id: String) -> JwstResult<bool> {
        self.db.check_blob(workspace, id).await
    }

    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstResult<Self::Read> {
        let blob = self
            .get_auto(workspace, id, params)
            .await
            .context("failed to get blob")?;
        Ok(ReaderStream::new(Cursor::new(blob)))
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<BlobMetadata> {
        self.db.get_metadata(workspace, id).await
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String> {
        self.db.put_blob(workspace, stream).await
    }

    async fn delete_blob(&self, workspace_id: Option<String>, id: String) -> JwstResult<bool> {
        // delete origin blobs
        let success = self
            .db
            .delete_blob(workspace_id.clone(), id.clone())
            .await?;

        // delete optimized blobs
        let workspace_id = workspace_id.unwrap_or("__default__".into());
        Ok(self
            .delete(&workspace_id, &id)
            .await
            .context("failed to delete optimized blob")?
            && success)
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<()> {
        // delete origin blobs
        self.db.delete_workspace(workspace_id.clone()).await?;

        // delete optimized blobs
        self.drop(&workspace_id)
            .await
            .context("failed to delete optimized blob")?;

        Ok(())
    }
}
