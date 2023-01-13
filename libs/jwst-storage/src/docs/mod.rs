mod entities;
mod filesystem;
#[cfg(feature = "mysql")]
mod mysql;
mod orm;
#[cfg(feature = "sqlite")]
mod sqlite;

use super::*;

#[cfg(feature = "mysql")]
pub use mysql::{MySQL as DocMySQLStorage, UpdateBinary as DocUpdateBinary};
#[cfg(feature = "sqlite")]
pub use sqlite::{SQLite as DocSQLiteStorage, UpdateBinary as DocUpdateBinary};

pub use entities::update_binary::Model as UpdateBinary;
pub use filesystem::FileSystem as DocFsStorage;
pub use orm::ORM as DocAutoStorage;
