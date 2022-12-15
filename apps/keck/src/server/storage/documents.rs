use super::types::DatabasePool;
use jwst_logger::{info, warn};
use sqlx::{query, query_as, Error};
use std::panic::{catch_unwind, AssertUnwindSafe};
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

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct UpdateBinary {
    pub id: i64,
    pub blob: Vec<u8>,
}

pub struct DocDatabase {
    pool: DatabasePool,
}

impl DocDatabase {
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
        DatabasePool::connect_with(options)
            .await
            .map(|pool| Self { pool })
    }

    #[cfg(all(test, feature = "jwst"))]
    pub async fn init_memory_pool() -> Result<Self, Error> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let path = format!("sqlite::memory:");
        let options = SqliteConnectOptions::from_str(&path)?.create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        Ok(Self { pool })
    }

    #[cfg(feature = "mysc")]
    pub async fn init_pool(database: &str) -> Result<Self, Error> {
        let env = dotenvy::var("DATABASE_URL")
            .unwrap_or_else(|_| format!("mysql://localhost/{}", database.to_string()));
        DatabasePool::connect(&env).await.map(|pool| Self { pool })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    async fn all(&self, table: &str) -> Result<Vec<UpdateBinary>, Error> {
        let stmt = format!("SELECT * FROM {}", table);
        let ret = query_as::<_, UpdateBinary>(&stmt)
            .fetch_all(&self.pool)
            .await?;
        Ok(ret)
    }

    async fn count(&self, table: &str) -> Result<i64, Error> {
        #[derive(sqlx::FromRow)]
        struct Count(i64);

        let stmt = format!("SELECT count(*) FROM {}", table);
        let ret = query_as::<_, Count>(&stmt).fetch_one(&self.pool).await?;
        Ok(ret.0)
    }

    async fn create(&self, table: &str) -> Result<(), Error> {
        #[cfg(feature = "jwst")]
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
            table
        );
        #[cfg(feature = "mysc")]
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (`id` INTEGER AUTO_INCREMENT, `blob` BLOB, PRIMARY KEY (id));",
            table
        );
        query(&stmt).execute(&self.pool).await?;
        Ok(())
    }

    async fn insert(&self, table: &str, blob: &[u8]) -> Result<(), Error> {
        let stmt = format!("INSERT INTO {} (`blob`) VALUES (?);", table);
        query(&stmt).bind(blob).execute(&self.pool).await?;
        Ok(())
    }

    async fn replace_with(&self, table: &str, blob: Vec<u8>) -> Result<(), Error> {
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
        if self.count(table).await? > 1 {
            self.replace_with(table, blob).await
        } else {
            Ok(())
        }
    }

    pub async fn create_doc(&self, workspace: &str) -> Result<Doc, Error> {
        let mut doc = Doc::with_options(Options {
            skip_gc: true,
            ..Default::default()
        });

        self.create(workspace).await?;

        let all_data = self.all(workspace).await?;

        if all_data.is_empty() {
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            self.insert(workspace, &update).await?;
        } else {
            doc = migrate_update(all_data, doc);
        }

        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        #[cfg(feature = "jwst")]
        let pool = DocDatabase::init_memory_pool().await?;
        #[cfg(feature = "mysql")]
        let pool = Database::init_pool("jwst").await?;
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
