#[cfg(feature = "mysc")]
pub use jwst_storage::{DocMySQLStorage as DocDatabase, UpdateBinary};
#[cfg(any(feature = "affine", feature = "jwst"))]
pub use jwst_storage::{DocSQLiteStorage as DocDatabase, UpdateBinary};

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
