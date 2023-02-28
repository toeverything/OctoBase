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
use std::{io::Cursor, path::PathBuf, sync::Arc};

pub use storage::JwstStorage;

type Bucket = Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>;

fn get_bucket() -> Bucket {
    Arc::new(RateLimiter::direct(
        Quota::per_second(nonzero!(30u32)).allow_burst(nonzero!(5u32)),
    ))
}
