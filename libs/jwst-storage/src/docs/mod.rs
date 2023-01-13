mod entities;
mod filesystem;
#[cfg(feature = "mysql")]
mod mysql;
mod orm;
#[cfg(feature = "sqlite")]
mod sqlite;

use super::*;

#[cfg(feature = "mysql")]
pub use mysql::MySQL as DocMySQLStorage;
#[cfg(feature = "sqlite")]
pub use sqlite::SQLite as DocSQLiteStorage;

pub use filesystem::FileSystem as DocFsStorage;
