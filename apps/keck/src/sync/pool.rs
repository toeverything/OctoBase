use jwst_logger::{info, warn};
use sqlx::{Error, SqlitePool};
use std::panic::{catch_unwind, AssertUnwindSafe};
use yrs::{updates::decoder::Decode, Doc, Options, StateVector, Update};

use super::{DbConn, UpdateBinary};

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
pub struct DbPool {
    #[cfg(not(feature = "mysql"))]
    pool: SqlitePool,
    #[cfg(feature = "mysql")]
    pool: sqlx::MySqlPool,
}

impl DbPool {
    #[cfg(not(feature = "mysql"))]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[cfg(feature = "mysql")]
    pub fn new(pool: sqlx::MySqlPool) -> Self {
        Self { pool }
    }

    async fn get_conn<'a>(&self, table: &'a str) -> Result<DbConn<'a>, Error> {
        Ok(DbConn {
            conn: self.pool.acquire().await?,
            table,
        })
    }

    pub async fn drop(&self, table: &str) -> Result<(), Error> {
        self.get_conn(table).await?.drop().await
    }

    pub async fn update(&self, table: &str, blob: Vec<u8>) -> Result<(), Error> {
        let mut conn = self.get_conn(table).await?;
        if conn.count().await? > MAX_TRIM_UPDATE_LIMIT - 1 {
            let data = conn.all().await?;

            let doc = migrate_update(data, Doc::default());

            let data = doc.encode_state_as_update_v1(&StateVector::default());

            conn.replace_with(data).await?;
        } else {
            conn.insert(&blob).await?;
        }

        Ok(())
    }

    pub async fn full_migrate(&self, table: &str, blob: Vec<u8>) -> Result<(), Error> {
        let mut conn = self.get_conn(table).await?;
        if conn.count().await? > 1 {
            conn.replace_with(blob).await
        } else {
            Ok(())
        }
    }

    pub async fn create_doc(&self, workspace: &str) -> Result<Doc, Error> {
        let mut doc = Doc::with_options(Options {
            skip_gc: true,
            ..Default::default()
        });

        let mut conn = self.get_conn(workspace).await?;

        conn.create().await?;

        let all_data = conn.all().await.unwrap();

        if all_data.is_empty() {
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            conn.insert(&update).await.unwrap();
        } else {
            doc = migrate_update(all_data, doc);
        }

        Ok(doc)
    }
}
