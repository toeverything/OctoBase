use super::*;
use jwst_logger::{info, warn};
use path_ext::PathExt;
use sqlx::{query, query_as, Error, SqlitePool};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
};
use yrs::{updates::decoder::Decode, Doc, Options, StateVector, Update};

const MAX_TRIM_UPDATE_LIMIT: i64 = 500;

fn migrate_update(updates: Vec<UpdateBinary>, doc: Doc) -> Doc {
    let mut trx = doc.transact();
    for update in updates {
        let id = update.id;
        match Update::decode_v1(&update.blob) {
            Ok(update) => {
                if let Err(e) = catch_unwind(AssertUnwindSafe(|| trx.apply_update(update))) {
                    warn!("update {} merge failed, skip it: {:?}", id, e);
                }
            }
            Err(err) => info!("failed to decode update: {:?}", err),
        }
    }
    trx.commit();

    doc
}

pub struct SQLite {
    pool: SqlitePool,
}

impl SQLite {
    pub async fn init_pool_with_name(file: &str) -> Result<Self, Error> {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
        use std::fs::create_dir;
        use std::str::FromStr;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data)?;
        }
        let path = format!(
            "sqlite:{}",
            data.join(PathBuf::from(file).name_str()).display()
        );

        let options = SqliteConnectOptions::from_str(&path)?
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);
        SqlitePool::connect_with(options)
            .await
            .map(|pool| Self { pool })
    }

    pub async fn init_pool_with_full_path(path: PathBuf) -> Result<Self, Error> {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
        use std::str::FromStr;

        let path = format!("sqlite:{}", path.display());
        let options = SqliteConnectOptions::from_str(&path)?
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);
        SqlitePool::connect_with(options)
            .await
            .map(|pool| Self { pool })
    }

    pub async fn init_memory_pool() -> Result<Self, Error> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?.create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;
        Ok(Self { pool })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn all(&self, table: &str) -> Result<Vec<UpdateBinary>, Error> {
        let stmt = format!("SELECT * FROM {}", table);
        let ret = query_as::<_, UpdateBinary>(&stmt)
            .fetch_all(&self.pool)
            .await?;
        Ok(ret)
    }

    pub async fn count(&self, table: &str) -> Result<i64, Error> {
        #[derive(sqlx::FromRow)]
        struct Count(i64);

        let stmt = format!("SELECT count(*) FROM {}", table);
        let ret = query_as::<_, Count>(&stmt).fetch_one(&self.pool).await?;
        Ok(ret.0)
    }

    pub async fn create(&self, table: &str) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
            table
        );
        query(&stmt).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn insert(&self, table: &str, blob: &[u8]) -> Result<(), Error> {
        let stmt = format!("INSERT INTO {} (`blob`) VALUES (?);", table);
        query(&stmt).bind(blob).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn replace_with(&self, table: &str, blob: Vec<u8>) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;

        let stmt = format!("DELETE FROM {}", table);
        query(&stmt).execute(&mut tx).await?;

        let stmt = format!("INSERT INTO {} (`blob`) VALUES (?);", table);
        query(&stmt).bind(blob).execute(&mut tx).await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn drop(&self, table: &str) -> Result<(), Error> {
        let stmt = format!("DROP TABLE IF EXISTS {};", table);
        query(&stmt).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn update(&self, table: &str, blob: Vec<u8>) -> Result<(), Error> {
        if self.count(table).await? > MAX_TRIM_UPDATE_LIMIT - 1 {
            let data = self.all(table).await?;

            let doc = migrate_update(data, Doc::default());

            let data = doc.encode_state_as_update_v1(&StateVector::default());

            self.replace_with(table, data).await?;
        } else {
            self.insert(table, &blob).await?;
        }

        Ok(())
    }

    pub async fn full_migrate(&self, table: &str, blob: Vec<u8>) -> Result<(), Error> {
        if self.count(table).await? > 0 {
            self.replace_with(table, blob).await
        } else {
            Ok(())
        }
    }

    pub async fn create_doc(&self, workspace_id: &str) -> Result<Doc, Error> {
        let mut doc = Doc::with_options(Options {
            skip_gc: true,
            ..Default::default()
        });

        self.create(workspace_id).await?;

        let all_data = self.all(workspace_id).await?;

        if all_data.is_empty() {
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            self.insert(workspace_id, &update).await?;
        } else {
            doc = migrate_update(all_data, doc);
        }

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage for SQLite {
    async fn get(&self, workspace_id: String) -> io::Result<Doc> {
        self.create_doc(&workspace_id)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_doc(&self, workspace_id: String, doc: &Doc) -> io::Result<()> {
        let data = doc.encode_state_as_update_v1(&StateVector::default());

        self.full_migrate(&workspace_id, data)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> io::Result<bool> {
        self.update(&workspace_id, data.into())
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(true)
    }

    async fn delete(&self, workspace_id: String) -> io::Result<()> {
        self.drop(&workspace_id)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        let pool = SQLite::init_memory_pool().await?;
        pool.create("basic").await?;

        // empty table
        assert_eq!(pool.count("basic").await?, 0);

        // first insert
        pool.insert("basic", &[1, 2, 3, 4]).await?;
        assert_eq!(pool.count("basic").await?, 1);

        // second insert
        pool.replace_with("basic", vec![2, 2, 3, 4]).await?;

        assert_eq!(
            pool.all("basic").await?,
            vec![UpdateBinary {
                id: 2,
                blob: vec![2, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        pool.drop("basic").await?;
        pool.create("basic").await?;
        pool.insert("basic", &[1, 2, 3, 4]).await?;
        assert_eq!(
            pool.all("basic").await?,
            vec![UpdateBinary {
                id: 1,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        Ok(())
    }
}
