#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;

pub mod model;

#[cfg(feature = "postgres")]
pub use postgres::PostgreSQL as PostgresDBContext;
#[cfg(feature = "sqlite")]
pub use sqlite::SQLite as SqliteDBContext;
