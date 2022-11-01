mod pool;
mod sync;

pub use pool::*;
pub use sync::*;

#[cfg(feature = "sqlite")]
type DatabasePool = sqlx::SqlitePool;

#[cfg(feature = "mysql")]
type DatabasePool = sqlx::MySqlPool;
