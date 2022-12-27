use super::*;
use chrono::Utc;
use sqlx::{query, query_as, Error};
use std::{
    collections::HashSet,
    io::Cursor,
    sync::{Arc, RwLock},
};
use tokio::io;

type DatabasePool = sqlx::SqlitePool;

pub struct SQLite {
    pool: DatabasePool,
    workspaces: Arc<RwLock<HashSet<String>>>,
}

impl SQLite {
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

    pub async fn init_memory_pool() -> Result<SQLite, Error> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?.create_if_missing(true);
        let pool = DatabasePool::connect_with(options).await?;
        Ok(SQLite {
            pool,
            workspaces: Arc::default(),
        })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn create(&self, table: &str) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                hash TEXT PRIMARY KEY,
                blob BLOB,
                created_at TIMESTAMP
            );",
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

    pub async fn metadata(&self, table: &str, hash: &str) -> Result<BlobMetadata, Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        #[derive(sqlx::FromRow)]
        struct Metadata {
            size: i64,
            created_at: i64,
        }

        let stmt = format!(
            "SELECT length(blob) as size, created_at from {} where hash = ?",
            table
        );
        let ret = query_as::<_, Metadata>(&stmt)
            .bind(hash)
            .fetch_one(&self.pool)
            .await?;

        Ok(BlobMetadata {
            size: ret.size as u64,
            last_modified: chrono::NaiveDateTime::from_timestamp_millis(ret.created_at).unwrap(),
        })
    }

    pub async fn insert(&self, table: &str, hash: &str, blob: &[u8]) -> Result<(), Error> {
        if !self.workspaces.read().unwrap().contains(table) {
            self.create(table).await?
        }

        if !self.exists(table, hash).await? {
            let stmt = format!(
                "INSERT INTO {} (`hash`, `blob`, `created_at`) VALUES (?, ?, ?);",
                table
            );
            query(&stmt)
                .bind(hash)
                .bind(blob)
                .bind(Utc::now().timestamp_millis())
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

#[async_trait]
impl BlobStorage for SQLite {
    type Read = ReaderStream<Cursor<Vec<u8>>>;

    async fn get_blob(&self, workspace: Option<String>, id: String) -> io::Result<Self::Read> {
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(blob) = self.get(&workspace, &id).await {
            return Ok(ReaderStream::new(Cursor::new(blob.blob)));
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
    }
    async fn get_metadata(&self, workspace: String, id: String) -> io::Result<BlobMetadata> {
        if let Ok(metadata) = self.metadata(&workspace, &id).await {
            Ok(metadata)
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
        }
    }
    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> io::Result<String> {
        let workspace = workspace.unwrap_or("__default__".into());

        let (hash, blob) = get_hash(stream).await;

        if self.insert(&workspace, &hash, &blob).await.is_ok() {
            Ok(hash)
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
        }
    }
    async fn delete_blob(&self, workspace: Option<String>, id: String) -> io::Result<()> {
        let workspace = workspace.unwrap_or("__default__".into());
        if let Ok(_success) = self.delete(&workspace, &id).await {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
        }
    }
    async fn delete_workspace(&self, workspace: String) -> io::Result<()> {
        if self.drop(&workspace).await.is_ok() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        let pool = BlobSQLiteStorage::init_memory_pool().await?;
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

        let metadata = pool.metadata("basic", "test1").await?;

        assert_eq!(metadata.size, 4);
        assert_eq!(metadata.last_modified.timestamp(), Utc::now().timestamp());

        Ok(())
    }
}
