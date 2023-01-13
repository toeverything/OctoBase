mod entities;
mod filesystem;

#[cfg(all(feature = "sqlite", feature = "mysql"))]
compile_error!(
    "Both features \"sqlite\" and \"mysql\" may not be enabled at the same time for this crate."
);

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

pub use filesystem::FileSystem as DocFsStorage;
