use sqlx::{query, query_as, Connection, Error, SqliteConnection};

#[derive(sqlx::FromRow, Debug)]
pub struct Update {
    id: i64,
    blob: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Count(i64);

pub struct SQLite {
    conn: SqliteConnection,
}

impl SQLite {
    pub async fn select_all(&mut self, idx: i64) -> Result<Vec<Update>, Error> {
        let ret = query_as::<_, Update>("SELECT * FROM updates where id >= ?")
            .bind(idx)
            .fetch_all(&mut self.conn)
            .await?;
        Ok(ret)
    }

    pub async fn count(&mut self) -> Result<i64, Error> {
        let ret = query_as::<_, Count>("SELECT count(*) FROM updates")
            .fetch_one(&mut self.conn)
            .await?;
        Ok(ret.0)
    }

    pub async fn insert(&mut self, blob: Vec<u8>) -> Result<(), Error> {
        query("INSERT INTO updates VALUES (null, ?);")
            .bind(blob)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }
}

pub async fn init() -> Result<SQLite, Error> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;

    query("CREATE TABLE IF NOT EXISTS updates (id INTEGER PRIMARY KEY AUTOINCREMENT, blob BLOB);")
        .execute(&mut conn)
        .await?;

    Ok(SQLite { conn })
}

#[tokio::test]
async fn sync_storage_test() -> anyhow::Result<()> {
    let mut sqlite = init().await?;

    sqlite.insert(vec![1, 2, 3, 4]).await?;
    println!("count: {}", sqlite.count().await?);

    println!("data: {:?}", sqlite.select_all(0).await?);

    Ok(())
}
