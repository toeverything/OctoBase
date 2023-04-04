mod entities;
mod rate_limiter;
mod storage;

use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use futures::{Future, Stream};
use jwst::{DocStorage, JwstError, JwstResult, Workspace};
use jwst_logger::{debug, error, info, trace, warn};
use path_ext::PathExt;
use rate_limiter::{get_bucket, is_sqlite, Bucket};
use sea_orm::{prelude::*, ConnectOptions, Database, DbErr, QuerySelect, Set};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub use storage::JwstStorage;

#[inline]
async fn create_connection(database: &str, single_thread: bool) -> JwstResult<DatabaseConnection> {
    Ok(Database::connect(
        ConnectOptions::from(database)
            .max_connections(if single_thread { 1 } else { 50 })
            .min_connections(if single_thread { 1 } else { 10 })
            .acquire_timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(5))
            .max_lifetime(Duration::from_secs(30))
            .to_owned(),
    )
    .await
    .context("Failed to connect to database")?)
}
