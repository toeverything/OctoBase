mod filesystem;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "sqlite")]
mod sqlite;

use super::*;
use base64::{
    alphabet::URL_SAFE,
    engine::{general_purpose::PAD, GeneralPurpose},
    Engine,
};
use bytes::Bytes;
use futures::{
    stream::{iter, StreamExt},
    Stream,
};
use sha2::{Digest, Sha256};
use tokio_util::io::ReaderStream;

const URL_SAFE_ENGINE: GeneralPurpose = GeneralPurpose::new(&URL_SAFE, PAD);

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

    let hash = URL_SAFE_ENGINE.encode(hasher.finalize());
    (hash, buffer)
}
