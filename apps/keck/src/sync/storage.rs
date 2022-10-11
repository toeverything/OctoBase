use sqlx::{
    pool::PoolConnection,
    query, query_as,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteJournalMode},
    Error, SqlitePool,
};
use std::str::FromStr;

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct UpdateBinary {
    pub id: i64,
    pub blob: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Count(i64);

pub(super) struct DbConn<'a> {
    pub(super) conn: PoolConnection<Sqlite>,
    pub(super) table: &'a str,
}

impl<'a> DbConn<'a> {
    pub async fn all(&mut self) -> Result<Vec<UpdateBinary>, Error> {
        let stmt = format!("SELECT * FROM {}", self.table);
        let ret = query_as::<_, UpdateBinary>(&stmt)
            .fetch_all(&mut self.conn)
            .await?;
        Ok(ret)
    }

    pub async fn count(&mut self) -> Result<i64, Error> {
        let stmt = format!("SELECT count(*) FROM {}", self.table);
        let ret = query_as::<_, Count>(&stmt)
            .fetch_one(&mut self.conn)
            .await?;
        Ok(ret.0)
    }

    pub async fn insert(&mut self, blob: &[u8]) -> Result<(), Error> {
        let stmt = format!("INSERT INTO {} VALUES (null, ?);", self.table);
        query(&stmt).bind(blob).execute(&mut self.conn).await?;
        Ok(())
    }

    pub async fn create(&mut self) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
            self.table
        );
        query(&stmt).execute(&mut self.conn).await?;
        Ok(())
    }
    pub async fn drop(&mut self) -> Result<(), Error> {
        let stmt = format!("DROP TABLE IF EXISTS {};", self.table);
        query(&stmt).execute(&mut self.conn).await?;
        Ok(())
    }

    pub async fn replace_with(&mut self, blob: Vec<u8>) -> Result<(), Error> {
        let stmt = format!(
            r#"
        BEGIN TRANSACTION;
        DELETE FROM {};
        INSERT INTO {} VALUES (null, ?);
        COMMIT;
        ;"#,
            self.table, self.table
        );

        query(&stmt)
            .bind(blob)
            .execute(&mut self.conn)
            .await
            .map(|_| ())
    }
}

pub async fn init_pool<F: ToString>(file: F) -> Result<SqlitePool, Error> {
    use std::fs::create_dir;
    use std::path::PathBuf;
    let data = PathBuf::from("data");
    if !data.exists() {
        create_dir(data)?;
    }
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        async fn init_memory_pool() -> anyhow::Result<SqlitePool> {
            let path = format!("sqlite::memory:");
            let options = SqliteConnectOptions::from_str(&path)?
                .journal_mode(SqliteJournalMode::Wal)
                .create_if_missing(true);
            Ok(SqlitePool::connect_with(options).await?)
        }

        let conn = init_memory_pool().await?.acquire().await?;
        let mut sqlite = DbConn {
            conn,
            table: &"basic",
        };
        sqlite.create().await?;

        // empty table
        assert_eq!(sqlite.count().await?, 0);

        // first insert
        sqlite.insert(&[1, 2, 3, 4]).await?;
        assert_eq!(sqlite.count().await?, 1);

        // second insert
        sqlite.replace_with(vec![2, 2, 3, 4]).await?;
        assert_eq!(
            sqlite.all().await?,
            vec![UpdateBinary {
                id: 2,
                blob: vec![2, 2, 3, 4]
            }]
        );
        assert_eq!(sqlite.count().await?, 1);

        sqlite.drop().await?;
        sqlite.create().await?;
        sqlite.insert(&[1, 2, 3, 4]).await?;
        assert_eq!(
            sqlite.all().await?,
            vec![UpdateBinary {
                id: 1,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(sqlite.count().await?, 1);

        Ok(())
    }
}
