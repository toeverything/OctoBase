mod entities;
mod storage;

use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{Future, Stream};
use governor::{
    clock::{QuantaClock, QuantaInstant},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
};
use governor::{Quota, RateLimiter};
use jwst::{DocStorage, JwstError, JwstResult, Workspace};
use jwst_logger::{debug, error, info, trace, warn};
use path_ext::PathExt;
use sea_orm::{prelude::*, ConnectOptions, Database, DbErr, FromQueryResult, QuerySelect, Set};
use std::{io::Cursor, num::NonZeroU32, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use url::Url;

pub use storage::JwstStorage;

pub struct Bucket {
    bucket: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>,
    semaphore: Arc<Semaphore>,
}

impl Bucket {
    fn new(bucket_size: u32, semaphore_size: usize) -> Self {
        let bucket_size =
            NonZeroU32::new(bucket_size).unwrap_or(unsafe { NonZeroU32::new_unchecked(1) });

        Self {
            bucket: Arc::new(RateLimiter::direct(
                Quota::per_second(bucket_size).allow_burst(bucket_size),
            )),
            semaphore: Arc::new(Semaphore::new(semaphore_size)),
        }
    }

    async fn get_lock(&self) -> OwnedSemaphorePermit {
        self.bucket.until_ready().await;
        self.semaphore.clone().acquire_owned().await.unwrap()
    }
}

#[inline]
fn is_sqlite(database: &str) -> bool {
    Url::parse(database)
        .map(|u| u.scheme() == "sqlite")
        .unwrap_or(false)
}

#[inline]
fn get_bucket(single_thread: bool) -> Arc<Bucket> {
    Arc::new(Bucket::new(
        if single_thread { 1 } else { 25 },
        if single_thread { 1 } else { 5 },
    ))
}

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
