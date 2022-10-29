use super::{Database, DatabasePool};
use sqlx::{pool::PoolConnection, query, query_as, Error};

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct UpdateBinary {
    pub id: i64,
    pub blob: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Count(i64);

cfg_if::cfg_if! {
    if #[cfg(feature = "sqlite")] {
        pub async fn init_pool<F: ToString>(file: F) -> Result<DatabasePool, Error> {
            use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
            use std::fs::create_dir;
            use std::path::PathBuf;
            use std::str::FromStr;

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
            DatabasePool::connect_with(options).await
        }
    } else if #[cfg(feature = "mysql")] {
        pub async fn init_pool<D: ToString>(database: D) -> Result<DatabasePool, Error> {
            let env = dotenvy::var("DATABASE_URL")
                .unwrap_or_else(|_| format!("mysql://localhost/{}", database.to_string()));
            DatabasePool::connect(&env).await
        }
    }
}

pub(super) struct DbConn<'a> {
    pub(super) conn: PoolConnection<Database>,
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
        let stmt = format!("INSERT INTO {} (blob) VALUES (?);", self.table);
        query(&stmt).bind(blob).execute(&mut self.conn).await?;
        Ok(())
    }

    pub async fn create(&mut self) -> Result<(), Error> {
        let stmt = if cfg!(feature = "sqlite") {
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
                self.table
            )
        } else if cfg!(feature = "mysql") {
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id INTEGER AUTO_INCREMENT, blob BLOB, PRIMARY KEY (id));",
                self.table
            )
        } else {
            unimplemented!("Unsupported database")
        };
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

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    async fn init_memory_pool() -> anyhow::Result<sqlx::SqlitePool> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?.create_if_missing(true);
        Ok(sqlx::SqlitePool::connect_with(options).await?)
    }

    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        let conn = init_memory_pool().await?.acquire().await?;
        let mut sqlite = DbConn {
            conn,
            table: "basic",
        };
        sqlite.create().await?;

        // empty table
        assert_eq!(sqlite.count().await?, 0);

        println!("{:?}", sqlite.all().await?);

        // first insert
        sqlite.insert(&[1, 2, 3, 4]).await?;
        assert_eq!(sqlite.count().await?, 1);
        println!("{:?}", sqlite.all().await?);

        // second insert
        sqlite.replace_with(vec![2, 2, 3, 4]).await?;
        println!("{:?}", sqlite.all().await?);
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
