use super::{entities::prelude::*, *};
use crate::types::JwstStorageResult;

// type DiffLogModel = <DiffLog as EntityTrait>::Model;
type DiffLogActiveModel = super::entities::diff_log::ActiveModel;
// type DiffLogColumn = <DiffLog as EntityTrait>::Column;

pub struct DiffLogRecord {
    bucket: Arc<Bucket>,
    pool: DatabaseConnection,
}

impl DiffLogRecord {
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

    pub async fn insert<C>(
        &self,
        workspace: String,
        ts: DateTime<Utc>,
        log: String,
    ) -> JwstStorageResult<()>
    where
        C: ConnectionTrait,
    {
        let _lock = self.bucket.write().await;
        DiffLog::insert(DiffLogActiveModel {
            workspace: Set(workspace),
            timestamp: Set(ts.into()),
            log: Set(log),
            ..Default::default()
        })
        .exec(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count(&self) -> JwstStorageResult<u64> {
        let _lock = self.bucket.read().await;
        let count = DiffLog::find().count(&self.pool).await?;
        Ok(count)
    }
}
