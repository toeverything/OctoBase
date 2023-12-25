#[forbid(unsafe_code)]
mod entities;
mod rate_limiter;
mod storage;
mod types;

use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use futures::Future;
use jwst_core::{DocStorage, JwstResult, Workspace};
use jwst_logger::{debug, error, info, trace, warn};
use path_ext::PathExt;
use rate_limiter::{get_bucket, is_sqlite, Bucket};
use sea_orm::{prelude::*, ConnectOptions, Database, DbErr, QuerySelect, Set};
#[cfg(feature = "bucket")]
pub use storage::blobs::MixedBucketDBParam;
pub use storage::{
    blobs::{stream_to_blob, BlobStorageType},
    JwstStorage,
};
pub use types::{JwstStorageError, JwstStorageResult};

#[inline]
async fn create_connection(database: &str, single_thread: bool) -> JwstStorageResult<DatabaseConnection> {
    let connection = Database::connect(
        ConnectOptions::from(database)
            .max_connections(if single_thread { 1 } else { 50 })
            .min_connections(if single_thread { 1 } else { 10 })
            .acquire_timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(5))
            .max_lifetime(Duration::from_secs(30))
            .to_owned(),
    )
    .await?;

    Ok(connection)
}
