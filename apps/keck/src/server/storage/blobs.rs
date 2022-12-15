use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use super::types::DatabasePool;
use sqlx::{query, query_as, Error};

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct BlobBinary {
    pub hash: String,
    pub blob: Vec<u8>,
}

pub struct BlobDatabase {
    pool: DatabasePool,
    workspaces: Arc<RwLock<HashSet<String>>>,
}

impl BlobDatabase {
    #[cfg(feature = "jwst")]
    pub async fn init_pool(file: &str) -> Result<Self, Error> {
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
        DatabasePool::connect_with(options).await.map(|pool| Self {
            pool,
            workspaces: Arc::default(),
        })
    }

    #[cfg(all(test, feature = "jwst"))]
    pub async fn init_memory_pool() -> Result<Self, Error> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?.create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        Ok(Self {
            pool,
            workspaces: Arc::default(),
        })
    }

    #[cfg(feature = "mysc")]
    pub async fn init_pool(database: &str) -> Result<Self, Error> {
        let env = dotenvy::var("DATABASE_URL")
            .unwrap_or_else(|_| format!("mysql://localhost/{}", database.to_string()));
        DatabasePool::connect(&env).await.map(|pool| Self {
            pool,
            workspaces: Arc::default(),
        })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn create(&self, table: &str) -> Result<(), Error> {
        #[cfg(feature = "jwst")]
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (hash TEXT PRIMARY KEY, blob BLOB);",
            table
        );
        #[cfg(feature = "mysc")]
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (`hash` VARCHAR(10) AUTO_INCREMENT, `blob` BLOB, PRIMARY KEY (hash));",
            table
        );
        query(&stmt).execute(&self.pool).await?;

        self.workspaces.write().unwrap().insert(table.into());
        Ok(())
    }

    pub async fn all(&self, table: &str) -> Result<Vec<BlobBinary>, Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        let stmt = format!("SELECT * FROM {}", table);
        let ret = query_as::<_, BlobBinary>(&stmt)
            .fetch_all(&self.pool)
            .await?;
        Ok(ret)
    }

    pub async fn count(&self, table: &str) -> Result<i64, Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        #[derive(sqlx::FromRow)]
        struct Count(i64);

        let stmt = format!("SELECT count(*) FROM {}", table);
        let ret = query_as::<_, Count>(&stmt).fetch_one(&self.pool).await?;
        Ok(ret.0)
    }

    pub async fn exists(&self, table: &str, hash: &str) -> Result<bool, Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        #[derive(sqlx::FromRow)]
        struct Count(i64);

        let stmt = format!("SELECT count(*) from {} where hash = ?", table);
        let ret = query_as::<_, Count>(&stmt)
            .bind(hash)
            .fetch_one(&self.pool)
            .await?;

        Ok(ret.0 == 1)
    }

    pub async fn insert(&self, table: &str, hash: &str, blob: &[u8]) -> Result<(), Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        if !self.exists(table, hash).await? {
            let stmt = format!("INSERT INTO {} (`hash`, `blob`) VALUES (?, ?);", table);
            query(&stmt)
                .bind(hash)
                .bind(blob)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn get(&self, table: &str, hash: &str) -> Result<BlobBinary, Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        let stmt = format!("SELECT * from {} where hash = ?", table);
        query_as(&stmt).bind(hash).fetch_one(&self.pool).await
    }

    pub async fn delete(&self, table: &str, hash: &str) -> Result<bool, Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        let stmt = format!("DELETE FROM {} where hash = ?", table);
        let ret = query(&stmt).bind(hash).execute(&self.pool).await?;
        Ok(ret.rows_affected() == 1)
    }

    pub async fn drop(&self, table: &str) -> Result<(), Error> {
        let stmt = format!("DROP TABLE IF EXISTS {};", table);
        query(&stmt).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        #[cfg(feature = "jwst")]
        let pool = BlobDatabase::init_memory_pool().await?;
        #[cfg(feature = "mysc")]
        let pool = Database::init_pool("jwst").await?;
        pool.create("basic").await?;

        // empty table
        assert_eq!(pool.count("basic").await?, 0);

        // first insert
        pool.insert("basic", "test", &[1, 2, 3, 4]).await?;
        assert_eq!(pool.count("basic").await?, 1);

        assert_eq!(
            pool.all("basic").await?,
            vec![BlobBinary {
                hash: "test".into(),
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        pool.drop("basic").await?;
        pool.create("basic").await?;
        pool.insert("basic", "test1", &[1, 2, 3, 4]).await?;
        assert_eq!(
            pool.all("basic").await?,
            vec![BlobBinary {
                hash: "test1".into(),
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        Ok(())
    }
}
