mod sqlite;

use super::*;

#[cfg(feature = "sqlite")]
type DatabasePool = sqlx::SqlitePool;
#[cfg(feature = "sqlite")]
pub use sqlite::BlobEmbeddedStorage;

#[cfg(feature = "mysql")]
type DatabasePool = sqlx::MySqlPool;
