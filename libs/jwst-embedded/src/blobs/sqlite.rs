use super::*;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{sink::Buffer, stream, stream::StreamExt, Stream};
use path_ext::PathExt;
use sqlx::{query, query_as, Error};
use std::{
    collections::HashSet,
    io::Cursor,
    path::Path,
    sync::{Arc, RwLock},
};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncRead, BufReader, BufStream},
    sync::SemaphorePermit,
};
use tokio_util::io::{ReaderStream, StreamReader};

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct BlobBinary {
    pub hash: String,
    pub blob: Vec<u8>,
}

pub struct BlobEmbeddedStorage {
    pool: DatabasePool,
    workspaces: Arc<RwLock<HashSet<String>>>,
}

impl BlobEmbeddedStorage {
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

    // async fn get_parallel(&self) -> Option<SemaphorePermit> {
    //     if let Some(m) = &self.max_parallel {
    //         Some(m.acquire().await.unwrap())
    //     } else {
    //         None
    //     }
    // }

    fn split_path(path: impl AsRef<Path> + Send) -> Option<[String; 2]> {
        if let [Some(name), Some(path)] = path.as_ref().components().fold(
            [Option::<String>::None, Option::<String>::None],
            |[name, path], c| {
                if name.is_none() {
                    [Some(c.full_str().into()), path]
                } else {
                    [
                        name,
                        path.map_or_else(
                            || Some(c.full_str().into()),
                            |s| format!("{}/{}", s, c.full_str()).into(),
                        ),
                    ]
                }
            },
        ) {
            Some([name, path])
        } else {
            None
        }
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn create(&self, table: &str) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (hash TEXT PRIMARY KEY, blob BLOB);",
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

#[async_trait]
impl BlobStorage for BlobEmbeddedStorage {
    type Read = ReaderStream<Cursor<Vec<u8>>>;

    async fn get(&self, path: impl AsRef<Path> + Send) -> io::Result<Self::Read> {
        if let Some([table, name]) = Self::split_path(path) {
            if let Ok(blob) = self.get(&table, &name).await {
                return Ok(ReaderStream::new(Cursor::new(blob.blob)));
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
    }

    async fn put(
        &self,
        stream: impl Stream<Item = Bytes> + Send,
        prefix: Option<String>,
    ) -> io::Result<String> {
        // let buffer =

        // self.put_file(
        //     &prefix
        //         .map(|prefix| self.path.join(prefix))
        //         .unwrap_or_else(|| self.path.to_path_buf()),
        //     stream,
        // )
        // .await
        Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
    }

    async fn rename(
        &self,
        from: impl AsRef<Path> + Send,
        to: impl AsRef<Path> + Send,
    ) -> io::Result<()> {
        // let _ = self.get_parallel().await;
        // fs::rename(self.path.join(from), self.path.join(to)).await
        Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
    }

    async fn delete(&self, path: impl AsRef<Path> + Send) -> io::Result<()> {
        // let _ = self.get_parallel().await;
        // fs::remove_file(self.path.join(path)).await
        Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
    }

    async fn get_metadata(&self, path: impl AsRef<Path> + Send) -> io::Result<BlobMetadata> {
        // self.count(table)
        // let last_modified = meta.modified()?;
        // let last_modifier: DateTime<Utc> = last_modified.into();

        // Ok(BlobMetadata {
        //     size: 0,                           // meta.len(),
        //     last_modified: Default::default(), //last_modifier.naive_utc(),
        // })
        Err(io::Error::new(io::ErrorKind::NotFound, "Not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn init_memory_pool() -> Result<BlobEmbeddedStorage, Error> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?.create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        Ok(BlobEmbeddedStorage {
            pool,
            workspaces: Arc::default(),
        })
    }

    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        let pool = init_memory_pool().await?;
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
