mod blobs;
mod docs;
mod entities;
mod storage;
mod tests;
mod utils;

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
use nonzero_ext::*;
use path_ext::PathExt;
use sea_orm::{prelude::*, Database, DbErr, FromQueryResult, QuerySelect, Set, TransactionTrait};
use std::{io::Cursor, num::NonZeroU32, path::PathBuf, sync::Arc};
use url::Url;

pub use storage::JwstStorage;

type Bucket = Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>;

#[inline]
fn is_sqlite(database: &str) -> bool {
    Url::parse(database)
        .map(|u| u.scheme() == "sqlite")
        .unwrap_or(false)
}

#[inline]
fn get_bucket(single_thread: bool) -> Bucket {
    Arc::new(RateLimiter::direct(
        Quota::per_second(nonzero!(30u32)).allow_burst(unsafe {
            NonZeroU32::new_unchecked(if single_thread { 1u32 } else { 5u32 })
        }),
    ))
}
