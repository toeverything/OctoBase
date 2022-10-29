mod pool;
mod storage;
mod sync;

pub use pool::*;
pub use storage::*;
pub use sync::*;

cfg_if::cfg_if! {
    if #[cfg(feature = "sqlite")] {
        type Database = sqlx::Sqlite;
        type DatabasePool = sqlx::SqlitePool;
    } else if #[cfg(feature = "mysql")] {
        type Database = sqlx::MySql;
        type DatabasePool = sqlx::MySqlPool;
    }
}
