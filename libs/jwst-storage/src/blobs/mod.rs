mod filesystem;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "sqlite")]
mod sqlite;

use super::*;
use bytes::Bytes;
use futures::{
    stream::{iter, StreamExt},
    Stream,
};
use sha2::{Digest, Sha256};
use tokio_util::io::ReaderStream;

const URL_SAFE_ENGINE: base64::engine::fast_portable::FastPortable =
    base64::engine::fast_portable::FastPortable::from(
        &base64::alphabet::URL_SAFE,
        base64::engine::fast_portable::NO_PAD,
    );

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct BlobBinary {
    pub hash: String,
    pub blob: Vec<u8>,
}

#[cfg(feature = "mysql")]
pub use mysql::MySQL as BlobMySQLStorage;
#[cfg(feature = "sqlite")]
pub use sqlite::SQLite as BlobSQLiteStorage;

pub use filesystem::FileSystem as BlobFsStorage;

async fn get_hash(stream: impl Stream<Item = Bytes> + Send) -> (String, Vec<u8>) {
    let mut hasher = Sha256::new();

    let buffer = stream
        .flat_map(|buffer| {
            hasher.update(&buffer);
            iter(buffer)
        })
        .collect()
        .await;

    let hash = base64::encode_engine(hasher.finalize(), &URL_SAFE_ENGINE);
    (hash, buffer)
}
