#[cfg(feature = "sqlite")]
pub type DatabasePool = sqlx::SqlitePool;

#[cfg(feature = "mysql")]
pub type DatabasePool = sqlx::MySqlPool;
