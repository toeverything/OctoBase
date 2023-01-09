mod filesystem;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "sqlite")]
mod sqlite;

use super::*;

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct UpdateBinary {
    pub id: i64,
    pub blob: Vec<u8>,
}

#[cfg(feature = "mysql")]
pub use mysql::MySQL as DocMySQLStorage;
#[cfg(feature = "sqlite")]
pub use sqlite::SQLite as DocSQLiteStorage;

pub use filesystem::FileSystem as DocFsStorage;
