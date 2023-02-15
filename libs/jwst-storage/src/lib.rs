mod blobs;
mod docs;
mod entities;
mod tests;
mod utils;

pub use blobs::BlobsAutoStorage as BlobAutoStorage;
pub use entities::blobs::Model as BlobBinary;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{Future, Stream};
use path_ext::PathExt;
use sea_orm::{prelude::*, Database, FromQueryResult, QuerySelect, Set, TransactionTrait};
use std::{
    io::{self, Cursor},
    path::PathBuf,
};

pub use blobs::*;
pub use docs::*;

#[derive(Clone)]
pub struct JwstStorage {
    pool: DatabaseConnection,
    blobs: BlobsAutoStorage,
    docs: DocAutoStorage,
}

impl JwstStorage {
    pub async fn new(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;

        let blobs = BlobsAutoStorage::init_with_pool(pool.clone()).await?;
        let docs = DocAutoStorage::init_with_pool(pool.clone()).await?;

        Ok(Self { pool, blobs, docs })
    }

    pub fn blobs(&self) -> &BlobsAutoStorage {
        &self.blobs
    }

    pub fn docs(&self) -> &DocAutoStorage {
        &self.docs
    }

    pub async fn with_pool<R, F, Fut>(&self, func: F) -> Result<R, DbErr>
    where
        F: Fn(DatabaseConnection) -> Fut,
        Fut: Future<Output = Result<R, DbErr>>,
    {
        func(self.pool.clone()).await
    }
}
