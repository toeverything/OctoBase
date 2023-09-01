use blob_storage::BlobDBStorage;
use bytes::Bytes;
use jwst_core::{BlobMetadata, BlobStorage};

use super::*;

pub(super) type OptimizedBlobModel = <OptimizedBlobs as EntityTrait>::Model;
type OptimizedBlobActiveModel = super::entities::optimized_blobs::ActiveModel;
type OptimizedBlobColumn = <OptimizedBlobs as EntityTrait>::Column;

#[derive(Clone)]
pub struct BlobAutoStorage {
    pub(crate) db: Arc<BlobDBStorage>,
    pool: DatabaseConnection,
}

impl BlobAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection, bucket: Arc<Bucket>) -> JwstStorageResult<Self> {
        let db = Arc::new(BlobDBStorage::init_with_pool(pool, bucket).await?);
        let pool = db.pool.clone();
        Ok(Self { db, pool })
    }

    pub async fn init_pool(database: &str) -> JwstStorageResult<Self> {
        let db = Arc::new(BlobDBStorage::init_pool(database).await?);
        let pool = db.pool.clone();
        Ok(Self { db, pool })
    }

    async fn exists(&self, table: &str, hash: &str, params: &str) -> JwstBlobResult<bool> {
        Ok(OptimizedBlobs::find_by_id((table.into(), hash.into(), params.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)?)
    }

    async fn insert(&self, table: &str, hash: &str, params: &str, blob: &[u8]) -> JwstBlobResult<()> {
        if !self.exists(table, hash, params).await? {
            OptimizedBlobs::insert(OptimizedBlobActiveModel {
                workspace_id: Set(table.into()),
                hash: Set(hash.into()),
                blob: Set(blob.into()),
                length: Set(blob.len().try_into().unwrap()),
                params: Set(params.into()),
                created_at: Set(Utc::now().into()),
            })
            .exec(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn get(&self, table: &str, hash: &str, params: &str) -> JwstBlobResult<OptimizedBlobModel> {
        OptimizedBlobs::find_by_id((table.into(), hash.into(), params.into()))
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn metadata(&self, table: &str, hash: &str, params: &str) -> JwstBlobResult<InternalBlobMetadata> {
        OptimizedBlobs::find_by_id((table.into(), hash.into(), params.into()))
            .select_only()
            .column_as(OptimizedBlobColumn::Length, "size")
            .column_as(OptimizedBlobColumn::CreatedAt, "created_at")
            .into_model::<InternalBlobMetadata>()
            .one(&self.pool)
            .await
            .map_err(|e| e.into())
            .and_then(|r| r.ok_or(JwstBlobError::BlobNotFound(hash.into())))
    }

    async fn get_metadata_auto(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstBlobResult<BlobMetadata> {
        let workspace_id = workspace.as_deref().unwrap_or("__default__");
        if let Some(params) = params {
            if let Ok(params) = ImageParams::try_from(&params) {
                let params_token = params.to_string();
                if self.exists(workspace_id, &id, &params_token).await? {
                    let metadata = self.metadata(workspace_id, &id, &params_token).await?;
                    Ok(BlobMetadata {
                        content_type: format!("image/{}", params.format()),
                        ..metadata.into()
                    })
                } else {
                    self.db.metadata(workspace_id, &id).await.map(Into::into)
                }
            } else {
                Err(JwstBlobError::ImageParams(params))
            }
        } else {
            self.db.metadata(workspace_id, &id).await.map(Into::into)
        }
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
                    let image = tokio::task::spawn_blocking(move || params.optimize_image(&blob.blob)).await??;
                    self.insert(workspace_id, &id, &params_token, &image).await?;
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
                Err(JwstBlobError::ImageParams(params))
            }
        } else {
            self.db.get(workspace_id, &id).await.map(|m| m.blob)
        }
    }

    async fn delete(&self, table: &str, hash: &str) -> JwstBlobResult<u64> {
        Ok(OptimizedBlobs::delete_many()
            .filter(
                OptimizedBlobColumn::WorkspaceId
                    .eq(table)
                    .and(OptimizedBlobColumn::Hash.eq(hash)),
            )
            .exec(&self.pool)
            .await
            .map(|r| r.rows_affected)?)
    }

    async fn drop(&self, table: &str) -> Result<(), DbErr> {
        OptimizedBlobs::delete_many()
            .filter(OptimizedBlobColumn::WorkspaceId.eq(table))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl BlobStorage<JwstStorageError> for BlobAutoStorage {
    async fn list_blobs(&self, workspace: Option<String>) -> JwstStorageResult<Vec<String>> {
        self.db.list_blobs(workspace).await
    }

    async fn check_blob(&self, workspace: Option<String>, id: String) -> JwstStorageResult<bool> {
        self.db.check_blob(workspace, id).await
    }

    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstStorageResult<Vec<u8>> {
        let blob = self.get_auto(workspace, id, params).await?;
        Ok(blob)
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
        params: Option<HashMap<String, String>>,
    ) -> JwstStorageResult<BlobMetadata> {
        let metadata = self.get_metadata_auto(workspace, id, params).await?;
        Ok(metadata)
    }

    async fn put_blob_stream(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstStorageResult<String> {
        self.db.put_blob_stream(workspace, stream).await
    }

    async fn put_blob(&self, workspace: Option<String>, blob: Vec<u8>) -> JwstStorageResult<String> {
        self.db.put_blob(workspace, blob).await
    }

    async fn delete_blob(&self, workspace_id: Option<String>, id: String) -> JwstStorageResult<bool> {
        // delete origin blobs
        let success = self.db.delete_blob(workspace_id.clone(), id.clone()).await?;
        if success {
            // delete optimized blobs
            let workspace_id = workspace_id.unwrap_or("__default__".into());
            self.delete(&workspace_id, &id).await?;
        }
        Ok(success)
    }

    async fn delete_workspace(&self, workspace_id: String) -> JwstStorageResult<()> {
        // delete origin blobs
        self.db.delete_workspace(workspace_id.clone()).await?;

        // delete optimized blobs
        self.drop(&workspace_id).await?;

        Ok(())
    }

    async fn get_blobs_size(&self, workspaces: Vec<String>) -> JwstStorageResult<i64> {
        let size = self.db.get_blobs_size(&workspaces).await?;

        return Ok(size.unwrap_or(0));
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use futures::FutureExt;
    use image::{DynamicImage, ImageOutputFormat};

    use super::*;

    #[tokio::test]
    async fn test_blob_auto_storage() {
        let storage = BlobAutoStorage::init_pool("sqlite::memory:").await.unwrap();
        Migrator::up(&storage.pool, None).await.unwrap();

        assert_eq!(storage.get_blobs_size(vec!["blob".into()]).await.unwrap(), 0);

        let blob = Vec::from_iter((0..100).map(|_| rand::random()));

        let stream = async { Bytes::from(blob.clone()) }.into_stream();
        let hash1 = storage.put_blob_stream(Some("blob".into()), stream).await.unwrap();

        // check origin blob result
        assert_eq!(
            storage
                .get_blob(Some("blob".into()), hash1.clone(), None)
                .await
                .unwrap(),
            blob
        );
        assert_eq!(
            storage
                .get_metadata(Some("blob".into()), hash1.clone(), None)
                .await
                .unwrap()
                .size as usize,
            blob.len()
        );
        assert_eq!(
            storage.get_blobs_size(vec!["blob".into()]).await.unwrap(),
            blob.len() as i64
        );

        // optimize must failed if blob not supported
        assert!(storage
            .get_blob(
                Some("blob".into()),
                hash1.clone(),
                Some(HashMap::from([("format".into(), "jpeg".into())]))
            )
            .await
            .is_err());

        // generate image
        let image = {
            let mut image = Cursor::new(vec![]);
            DynamicImage::new_rgba8(32, 32)
                .write_to(&mut image, ImageOutputFormat::Png)
                .unwrap();
            image.into_inner()
        };
        let stream = async { Bytes::from(image.clone()) }.into_stream();
        let hash2 = storage.put_blob_stream(Some("blob".into()), stream).await.unwrap();

        // check origin blob result
        assert_eq!(
            storage
                .get_blob(Some("blob".into()), hash2.clone(), None)
                .await
                .unwrap(),
            image
        );
        assert_eq!(
            storage
                .get_metadata(Some("blob".into()), hash2.clone(), None)
                .await
                .unwrap()
                .size as usize,
            image.len()
        );

        // check optimized jpeg result
        let jpeg_params = HashMap::from([("format".into(), "jpeg".into())]);
        let jpeg = storage
            .get_blob(Some("blob".into()), hash2.clone(), Some(jpeg_params.clone()))
            .await
            .unwrap();

        assert!(jpeg.starts_with(&[0xff, 0xd8, 0xff]));
        assert_eq!(
            storage
                .get_metadata(Some("blob".into()), hash2.clone(), Some(jpeg_params))
                .await
                .unwrap()
                .size as usize,
            jpeg.len()
        );

        // check optimized webp result
        let webp_params = HashMap::from([("format".into(), "webp".into())]);
        let webp = storage
            .get_blob(Some("blob".into()), hash2.clone(), Some(webp_params.clone()))
            .await
            .unwrap();

        assert!(webp.starts_with(b"RIFF"));
        assert_eq!(
            storage
                .get_metadata(Some("blob".into()), hash2.clone(), Some(webp_params.clone()))
                .await
                .unwrap()
                .size as usize,
            webp.len()
        );

        // optimize must failed if image params error
        assert!(storage
            .get_blob(
                Some("blob".into()),
                hash2.clone(),
                Some(HashMap::from([("format".into(), "error_value".into()),]))
            )
            .await
            .is_err());
        assert!(storage
            .get_blob(
                Some("blob".into()),
                hash2.clone(),
                Some(HashMap::from([
                    ("format".into(), "webp".into()),
                    ("size".into(), "error_value".into())
                ]))
            )
            .await
            .is_err());
        assert!(storage
            .get_blob(
                Some("blob".into()),
                hash2.clone(),
                Some(HashMap::from([
                    ("format".into(), "webp".into()),
                    ("width".into(), "111".into())
                ]))
            )
            .await
            .is_err());
        assert!(storage
            .get_blob(
                Some("blob".into()),
                hash2.clone(),
                Some(HashMap::from([
                    ("format".into(), "webp".into()),
                    ("height".into(), "111".into())
                ]))
            )
            .await
            .is_err());

        assert_eq!(
            storage.get_blobs_size(vec!["blob".into()]).await.unwrap(),
            (blob.len() + image.len()) as i64
        );

        assert!(storage.delete_blob(Some("blob".into()), hash2.clone()).await.unwrap());
        assert_eq!(
            storage.check_blob(Some("blob".into()), hash2.clone()).await.unwrap(),
            false
        );
        assert!(storage
            .get_blob(Some("blob".into()), hash2.clone(), None)
            .await
            .is_err());
        assert!(storage
            .get_metadata(Some("blob".into()), hash2.clone(), None)
            .await
            .is_err());
        assert!(storage
            .get_metadata(Some("blob".into()), hash2.clone(), Some(webp_params))
            .await
            .is_err());

        assert_eq!(storage.get_blobs_size(vec!["blob".into()]).await.unwrap() as usize, 100);

        assert_eq!(storage.list_blobs(Some("blob".into())).await.unwrap(), vec![hash1]);
        assert_eq!(
            storage.list_blobs(Some("not_exists_workspace".into())).await.unwrap(),
            Vec::<String>::new()
        );

        {
            let blob = Vec::from_iter((0..100).map(|_| rand::random()));
            let stream = async { Bytes::from(blob.clone()) }.into_stream();
            storage.put_blob_stream(Some("blob1".into()), stream).await.unwrap();

            assert_eq!(
                storage
                    .get_blobs_size(vec!["blob".into(), "blob1".into()])
                    .await
                    .unwrap() as usize,
                200
            );
        }

        // test calc with not exists workspaces
        {
            assert_eq!(
                storage
                    .get_blobs_size(vec!["blob".into(), "blob1".into(), "blob2".into()])
                    .await
                    .unwrap(),
                200
            );

            assert_eq!(storage.get_blobs_size(vec!["blob2".into()]).await.unwrap(), 0);
        }
    }
}
