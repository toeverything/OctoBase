use super::{entities::prelude::*, utils::get_hash, *};
use bytes::Bytes;
use jwst::{BlobMetadata, BlobStorage};
use jwst_storage_migration::{Migrator, MigratorTrait};
use tokio_util::io::ReaderStream;

pub(super) type BlobModel = <Blobs as EntityTrait>::Model;
type BlobActiveModel = super::entities::blobs::ActiveModel;
type BlobColumn = <Blobs as EntityTrait>::Column;

#[derive(Clone)]
pub struct BlobAutoStorage {
    pool: DatabaseConnection,
}

impl BlobAutoStorage {
    pub async fn init_with_pool(pool: DatabaseConnection) -> Result<Self, DbErr> {
        Migrator::up(&pool, None).await?;
        Ok(Self { pool })
    }

    pub async fn init_pool(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;
        Migrator::up(&pool, None).await?;
        Ok(Self { pool })
    }

    pub async fn init_sqlite_pool_with_name(file: &str) -> Result<Self, DbErr> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).map_err(|e| DbErr::Custom(e.to_string()))?;
        }

        Self::init_pool(&format!(
            "sqlite:{}?mode=rwc",
            data.join(PathBuf::from(file).name_str())
                .with_extension("db")
                .display()
        ))
        .await
    }

    pub async fn all(&self, table: &str) -> Result<Vec<BlobModel>, DbErr> {
        Blobs::find()
            .filter(BlobColumn::Workspace.eq(table))
            .all(&self.pool)
            .await
    }

    pub async fn count(&self, table: &str) -> Result<u64, DbErr> {
        Blobs::find()
            .filter(BlobColumn::Workspace.eq(table))
            .count(&self.pool)
            .await
    }

    pub async fn exists(&self, table: &str, hash: &str) -> Result<bool, DbErr> {
        Blobs::find_by_id((table.into(), hash.into()))
            .count(&self.pool)
            .await
            .map(|c| c > 0)
    }

    pub async fn metadata(&self, table: &str, hash: &str) -> Result<BlobMetadata, DbErr> {
        #[derive(FromQueryResult)]
        struct Metadata {
            size: i64,
            created_at: DateTime<Utc>,
        }

        let ret = Blobs::find_by_id((table.into(), hash.into()))
            .select_only()
            .column_as(BlobColumn::Length, "size")
            .column_as(BlobColumn::Timestamp, "created_at")
            .into_model::<Metadata>()
            .one(&self.pool)
            .await
            .and_then(|r| r.ok_or(DbErr::Query(RuntimeErr::Internal("blob not exists".into()))))?;

        Ok(BlobMetadata {
            size: ret.size as u64,
            last_modified: ret.created_at.naive_local(),
        })
    }

    pub async fn insert(&self, table: &str, hash: &str, blob: &[u8]) -> Result<(), DbErr> {
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

    pub async fn get(&self, table: &str, hash: &str) -> Result<BlobModel, DbErr> {
        Blobs::find_by_id((table.into(), hash.into()))
            .one(&self.pool)
            .await
            .and_then(|r| r.ok_or(DbErr::Query(RuntimeErr::Internal("blob not exists".into()))))
    }

    pub async fn delete(&self, table: &str, hash: &str) -> Result<bool, DbErr> {
        Blobs::delete_by_id((table.into(), hash.into()))
            .exec(&self.pool)
            .await
            .map(|r| r.rows_affected == 1)
    }

    pub async fn drop(&self, table: &str) -> Result<(), DbErr> {
        Blobs::delete_many()
            .filter(BlobColumn::Workspace.eq(table))
            .exec(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl BlobStorage for BlobAutoStorage {
    type Read = ReaderStream<Cursor<Vec<u8>>>;

    async fn get_blob(&self, workspace: Option<String>, id: String) -> JwstResult<Self::Read> {
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(blob) = self.get(&workspace, &id).await {
            return Ok(ReaderStream::new(Cursor::new(blob.blob)));
        }

        Err(JwstError::WorkspaceNotFound(workspace))
    }
    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> JwstResult<BlobMetadata> {
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(metadata) = self.metadata(&workspace, &id).await {
            Ok(metadata)
        } else {
            Err(JwstError::WorkspaceNotFound(workspace))
        }
    }
    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> JwstResult<String> {
        let workspace = workspace.unwrap_or("__default__".into());

        let (hash, blob) = get_hash(stream).await;

        if self.insert(&workspace, &hash, &blob).await.is_ok() {
            Ok(hash)
        } else {
            Err(JwstError::WorkspaceNotFound(workspace))
        }
    }
    async fn delete_blob(&self, workspace_id: Option<String>, id: String) -> JwstResult<()> {
        let workspace_id = workspace_id.unwrap_or("__default__".into());
        if let Ok(_success) = self.delete(&workspace_id, &id).await {
            Ok(())
        } else {
            Err(JwstError::WorkspaceNotFound(workspace_id))
        }
    }
    async fn delete_workspace(&self, workspace_id: String) -> JwstResult<()> {
        if self.drop(&workspace_id).await.is_ok() {
            Ok(())
        } else {
            Err(JwstError::WorkspaceNotFound(workspace_id))
        }
    }
}
