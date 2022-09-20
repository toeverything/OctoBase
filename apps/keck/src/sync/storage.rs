use futures::Future;
use sqlx::{
    pool::PoolConnection,
    query, query_as,
    sqlite::{Sqlite, SqliteConnectOptions, SqliteJournalMode},
    ConnectOptions, Connection, Error, SqliteConnection, SqlitePool,
};
use std::str::FromStr;

#[derive(sqlx::FromRow, Debug)]
pub struct UpdateBinary {
    id: i64,
    pub blob: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Count(i64);

pub struct SQLite {
    conn: SqlitePool,
    table: String,
}

impl SQLite {
    fn get_conn(&self) -> impl Future<Output = Result<PoolConnection<Sqlite>, Error>> {
        self.conn.acquire()
    }

    pub async fn all(&self, idx: i64) -> Result<Vec<UpdateBinary>, Error> {
        let stmt = format!("SELECT * FROM {} where id >= ?", self.table);
        let ret = query_as::<_, UpdateBinary>(&stmt)
            .bind(idx)
            .fetch_all(&mut self.get_conn().await?)
            .await?;
        Ok(ret)
    }

    pub async fn count(&self) -> Result<i64, Error> {
        let stmt = format!("SELECT count(*) FROM {}", self.table);
        let ret = query_as::<_, Count>(&stmt)
            .fetch_one(&mut self.get_conn().await?)
            .await?;
        Ok(ret.0)
    }

    pub async fn insert(&self, blob: &[u8]) -> Result<(), Error> {
        let stmt = format!("INSERT INTO {} VALUES (null, ?);", self.table);
        query(&stmt)
            .bind(blob)
            .execute(&mut self.get_conn().await?)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, idx: i64) -> Result<(), Error> {
        let stmt = format!("DELETE FROM {} WHERE id < ?", self.table);
        query(&stmt)
            .bind(idx)
            .execute(&mut self.get_conn().await?)
            .await?;
        Ok(())
    }

    pub async fn create(&self) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
            self.table
        );
        query(&stmt).execute(&mut self.get_conn().await?).await?;
        Ok(())
    }
    pub async fn drop(&self) -> Result<(), Error> {
        let stmt = format!("DROP TABLE {};", self.table);
        query(&stmt).execute(&mut self.get_conn().await?).await?;
        Ok(())
    }
}

pub async fn init<F: ToString, T: ToString>(file: F, table: T) -> Result<SQLite, Error> {
    let table = table.to_string();
    let path = format!(
        "sqlite:{}",
        std::env::current_dir()
            .unwrap()
            .join(format!("{}.db", file.to_string()))
            .display()
    );
    let options = SqliteConnectOptions::from_str(&path)?
        .journal_mode(SqliteJournalMode::Wal)
        .create_if_missing(true);
    let db = SQLite {
        conn: SqlitePool::connect_with(options).await?,
        table: table.to_string(),
    };
    db.create().await?;
    Ok(db)
}

#[tokio::test]
async fn sync_storage_test() -> anyhow::Result<()> {
    let sqlite = init("jwst", "updates").await?;

    sqlite.insert(&[1, 2, 3, 4]).await?;
    println!("count: {}", sqlite.count().await?);
    sqlite.insert(&[2, 2, 3, 4]).await?;
    sqlite.delete(2).await?;
    println!("data: {:?}", sqlite.all(0).await?);

    sqlite.drop().await?;
    sqlite.create().await?;
    sqlite.insert(&[1, 2, 3, 4]).await?;
    println!("data: {:?}", sqlite.all(0).await?);

    Ok(())
}
