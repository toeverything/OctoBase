use sqlx::{
    query, query_as,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    ConnectOptions, Connection, Error, SqliteConnection,
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
    conn: SqliteConnection,
    table: String,
}

impl SQLite {
    pub async fn all(&mut self, idx: i64) -> Result<Vec<UpdateBinary>, Error> {
        let stmt = format!("SELECT * FROM {} where id >= ?", self.table);
        let ret = query_as::<_, UpdateBinary>(&stmt)
            .bind(idx)
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
        let stmt = format!("INSERT INTO {} VALUES (null, ?);", self.table);
        query(&stmt).bind(blob).execute(&mut self.conn).await?;
        Ok(())
    }

    pub async fn delete(&mut self, idx: i64) -> Result<(), Error> {
        let stmt = format!("DELETE FROM {} WHERE id < ?", self.table);
        query(&stmt).bind(idx).execute(&mut self.conn).await?;
        Ok(())
    }

    pub async fn create(&mut self) -> Result<(), Error> {
        let stmt = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);",
            self.table
        );
        query(&stmt).execute(&mut self.conn).await?;
        Ok(())
    }
    pub async fn drop(&mut self) -> Result<(), Error> {
        let stmt = format!("DROP TABLE {};", self.table);
        query(&stmt).execute(&mut self.conn).await?;
        Ok(())
    }
}

pub async fn init<S>(table: S) -> Result<SQLite, Error>
where
    S: ToString,
{
    let table = table.to_string();
    let path = format!(
        "sqlite:{}",
        std::env::current_dir()
            .unwrap()
            .join(format!("{}.db", table))
            .display()
    );
    println!("{}", path);
    let conn = SqliteConnectOptions::from_str(&path)?
        .journal_mode(SqliteJournalMode::Wal)
        .create_if_missing(true)
        .connect()
        .await?;
    let mut db = SQLite {
        conn,
        table: table.to_string(),
    };
    db.create().await?;
    Ok(db)
}

#[tokio::test]
async fn sync_storage_test() -> anyhow::Result<()> {
    let mut sqlite = init("updates").await?;

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
