use super::*;
use jwst_logger::{info, warn};
use sqlx::{query, query_as, Error, MySqlPool};
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

pub struct MySQL {
    pool: MySqlPool,
}

impl MySQL {
    pub async fn init_pool(database: &str) -> Result<Self, Error> {
        let env = dotenvy::var("DATABASE_URL")
            .unwrap_or_else(|_| format!("mysql://localhost/{}", database.to_string()));
        MySqlPool::connect(&env).await.map(|pool| Self { pool })
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
            "CREATE TABLE IF NOT EXISTS {} (`id` INTEGER AUTO_INCREMENT, `blob` BLOB, PRIMARY KEY (id));",
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

#[async_trait]
impl DocStorage for MySQL {
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

    #[ignore = "need mysql server"]
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        let pool = MySQL::init_pool("jwst").await?;
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
