use super::{async_trait, entities::prelude::*, DocStorage};
use chrono::Utc;
use jwst_doc_migration::{Migrator, MigratorTrait};
use jwst_logger::{debug, info, warn};
use path_ext::PathExt;
use sea_orm::{prelude::*, Database, Set, TransactionTrait};
use std::{
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
};
use yrs::{updates::decoder::Decode, Doc, Options, StateVector, Update};

const MAX_TRIM_UPDATE_LIMIT: u64 = 500;

fn migrate_update(updates: Vec<<UpdateBinary as EntityTrait>::Model>, doc: Doc) -> Doc {
    let mut trx = doc.transact();
    for update in updates {
        let id = update.timestamp;
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

    debug!(
        "migrate_update: {:?}",
        doc.encode_state_as_update_v1(&StateVector::default())
    );

    doc
}

type UpdateBinaryModel = <UpdateBinary as EntityTrait>::Model;
type UpdateBinaryActiveModel = super::entities::update_binary::ActiveModel;
type UpdateBinaryColumn = <UpdateBinary as EntityTrait>::Column;

pub struct ORM {
    pool: DatabaseConnection,
}

impl ORM {
    pub async fn init_pool(database: &str) -> Result<Self, DbErr> {
        let pool = Database::connect(database).await?;
        Migrator::up(&pool, None).await?;
        Ok(Self { pool })
    }

    pub async fn init_sqlite_pool_with_name(file: &str) -> Result<Self, DbErr> {
        use std::fs::create_dir;

        let data = PathBuf::from("./data");
        if !data.exists() {
            create_dir(&data).map_err(|e| DbErr::Custom(e.to_string()))?;
        }

        Self::init_pool(&format!(
            "sqlite:{}?mode=rwc",
            data.join(PathBuf::from(file).name_str())
                .with_extension("db")
                .display()
        ))
        .await
    }

    pub async fn init_sqlite_pool_with_full_path(path: PathBuf) -> Result<Self, DbErr> {
        Self::init_pool(&format!("sqlite:{}?mode=rwc", path.display())).await
    }

    pub async fn all(&self, table: &str) -> Result<Vec<UpdateBinaryModel>, DbErr> {
        UpdateBinary::find()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .all(&self.pool)
            .await
    }

    async fn count(&self, table: &str) -> Result<u64, DbErr> {
        UpdateBinary::find()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .count(&self.pool)
            .await
    }

    async fn insert(&self, table: &str, blob: &[u8]) -> Result<(), DbErr> {
        UpdateBinary::insert(UpdateBinaryActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now()),
            blob: Set(blob.into()),
        })
        .exec(&self.pool)
        .await?;
        Ok(())
    }

    async fn replace_with(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        let mut tx = self.pool.begin().await?;

        UpdateBinary::delete_many()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .exec(&mut tx)
            .await?;

        UpdateBinary::insert(UpdateBinaryActiveModel {
            workspace: Set(table.into()),
            timestamp: Set(Utc::now()),
            blob: Set(blob.into()),
        })
        .exec(&mut tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn drop(&self, table: &str) -> Result<(), DbErr> {
        UpdateBinary::delete_many()
            .filter(UpdateBinaryColumn::Workspace.eq(table))
            .exec(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
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

    pub async fn full_migrate(&self, table: &str, blob: Vec<u8>) -> Result<(), DbErr> {
        if self.count(table).await? > 0 {
            self.replace_with(table, blob).await
        } else {
            self.insert(table, &blob).await
        }
    }

    pub async fn create_doc(&self, workspace: &str) -> Result<Doc, DbErr> {
        let mut doc = Doc::with_options(Options {
            skip_gc: true,
            ..Default::default()
        });

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
impl DocStorage for ORM {
    async fn get(&self, workspace_id: String) -> io::Result<Doc> {
        self.create_doc(&workspace_id)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_doc(&self, workspace_id: String, doc: &Doc) -> io::Result<()> {
        let data = doc.encode_state_as_update_v1(&StateVector::default());
        println!("write_doc");

        self.full_migrate(&workspace_id, data)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace_id: String, data: &[u8]) -> io::Result<bool> {
        println!("write_update");
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

        let pool = ORM::init_pool("sqlite::memory:").await?;

        // empty table
        assert_eq!(pool.count("basic").await?, 0);

        // first insert
        pool.insert("basic", &[1, 2, 3, 4]).await?;
        pool.insert("basic", &[2, 2, 3, 4]).await?;
        assert_eq!(pool.count("basic").await?, 2);

        // second insert
        pool.replace_with("basic", vec![3, 2, 3, 4]).await?;

        let all = pool.all("basic").await?;
        assert_eq!(
            all,
            vec![UpdateBinaryModel {
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![3, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        pool.drop("basic").await?;

        pool.insert("basic", &[1, 2, 3, 4]).await?;

        let all = pool.all("basic").await?;
        assert_eq!(
            all,
            vec![UpdateBinaryModel {
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        Ok(())
    }
}
