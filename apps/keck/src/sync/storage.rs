use futures::Future;
use sqlx::{
    pool::PoolConnection,
    query, query_as,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteJournalMode},
    Error, SqlitePool,
};
use std::str::FromStr;

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct UpdateBinary {
    id: i64,
    pub blob: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Count(i64);

#[derive(sqlx::FromRow)]
pub struct MaxId {
    seq: i64,
}

#[derive(Clone)]
pub struct SQLite {
    pub conn: SqlitePool,
    pub table: String,
}

impl SQLite {
    fn get_conn(&self) -> impl Future<Output = Result<PoolConnection<Sqlite>, Error>> {
        self.conn.acquire()
    }

    pub async fn all(&self, idx: i64) -> Result<Vec<UpdateBinary>, Error> {
        let stmt = format!("SELECT * FROM {} where id >= ?", self.table);
        let mut conn = self.get_conn().await?;
        let ret = query_as::<_, UpdateBinary>(&stmt)
            .bind(idx)
            .fetch_all(&mut conn)
            .await?;
        Ok(ret)
    }

    pub async fn count(&self) -> Result<i64, Error> {
        let stmt = format!("SELECT count(*) FROM {}", self.table);
        let mut conn = self.get_conn().await?;
        let ret = query_as::<_, Count>(&stmt).fetch_one(&mut conn).await?;
        Ok(ret.0)
    }

    pub async fn max_id(&self) -> Result<i64, Error> {
        let stmt = format!(
            "SELECT SEQ from sqlite_sequence WHERE name='{}'",
            self.table
        );
        let mut conn = self.get_conn().await?;
        let ret = query_as::<_, MaxId>(&stmt)
            .fetch_optional(&mut conn)
            .await?;
        Ok(ret.map(|ret| ret.seq).unwrap_or(0))
    }

    pub async fn insert(&self, blob: &[u8]) -> Result<(), Error> {
        let stmt = format!("INSERT INTO {} VALUES (null, ?);", self.table);
        let mut conn = self.get_conn().await?;
        query(&stmt).bind(blob).execute(&mut conn).await?;
        Ok(())
    }

    pub async fn delete_before(&self, idx: i64) -> Result<(), Error> {
        let stmt = format!("DELETE FROM {} WHERE id < ?", self.table);
        let mut conn = self.get_conn().await?;
        query(&stmt).bind(idx).execute(&mut conn).await?;
        Ok(())
    }

    pub async fn create(&self) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
            self.table
        );
        let mut conn = self.get_conn().await?;
        query(&stmt).execute(&mut conn).await?;
        Ok(())
    }
    pub async fn drop(&self) -> Result<(), Error> {
        let stmt = format!("DROP TABLE IF EXISTS {};", self.table);
        let mut conn = self.get_conn().await?;
        query(&stmt).execute(&mut conn).await?;
        Ok(())
    }
}

pub async fn init_pool<F: ToString>(file: F) -> Result<SqlitePool, Error> {
    use std::fs::create_dir;
    create_dir("data")?;
    let path = format!(
        "sqlite:{}",
        std::env::current_dir()
            .unwrap()
            .join(format!("./data/{}.db", file.to_string()))
            .display()
    );
    let options = SqliteConnectOptions::from_str(&path)?
        .journal_mode(SqliteJournalMode::Wal)
        .create_if_missing(true);
    SqlitePool::connect_with(options).await
}

pub async fn init<T: ToString>(conn: SqlitePool, table: T) -> Result<SQLite, Error> {
    let table = table.to_string();

    let db = SQLite {
        conn,
        table: table.to_string(),
    };
    db.create().await?;
    Ok(db)
}

mod tests {
    use super::*;

    async fn init_memory_pool() -> anyhow::Result<SqlitePool> {
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);
        Ok(SqlitePool::connect_with(options).await?)
    }

    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        let pool = init_memory_pool().await?;
        let sqlite = init(pool, "basic").await?;

        // empty table
        assert_eq!(sqlite.count().await?, 0);
        assert_eq!(sqlite.max_id().await?, 0);

        // first insert
        sqlite.insert(&[1, 2, 3, 4]).await?;
        assert_eq!(sqlite.count().await?, 1);
        assert_eq!(sqlite.max_id().await?, 1);

        // second insert
        sqlite.insert(&[2, 2, 3, 4]).await?;
        sqlite.delete_before(2).await?;
        assert_eq!(
            sqlite.all(0).await?,
            vec![UpdateBinary {
                id: 2,
                blob: vec![2, 2, 3, 4]
            }]
        );
        assert_eq!(sqlite.count().await?, 1);
        assert_eq!(sqlite.max_id().await?, 2);

        // clear table
        sqlite.delete_before(3).await?;
        assert_eq!(sqlite.count().await?, 0);
        assert_eq!(sqlite.max_id().await?, 2);

        sqlite.drop().await?;
        sqlite.create().await?;
        sqlite.insert(&[1, 2, 3, 4]).await?;
        assert_eq!(
            sqlite.all(0).await?,
            vec![UpdateBinary {
                id: 1,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(sqlite.count().await?, 1);
        assert_eq!(sqlite.max_id().await?, 1);

        Ok(())
    }
}
