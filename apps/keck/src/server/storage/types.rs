#[cfg(feature = "jwst")]
pub type DatabasePool = sqlx::SqlitePool;

#[cfg(feature = "mysc")]
pub type DatabasePool = sqlx::MySqlPool;
