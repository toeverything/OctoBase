mod filesystem;
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

#[cfg(feature = "sqlite")]
type DatabasePool = sqlx::SqlitePool;
#[cfg(feature = "sqlite")]
pub use sqlite::SQLite;

#[cfg(feature = "mysql")]
type DatabasePool = sqlx::MySqlPool;

pub use filesystem::FileSystem;
