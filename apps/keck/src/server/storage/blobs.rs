#[cfg(feature = "mysc")]
pub use jwst_storage::BlobMySQLStorage as BlobDatabase;
#[cfg(any(feature = "affine", feature = "jwst"))]
pub use jwst_storage::BlobSQLiteStorage as BlobDatabase;

pub use jwst_storage::BlobBinary;

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "jwst", feature = "mysql"))]
    #[tokio::test]
    async fn basic_storage_test() -> anyhow::Result<()> {
        use super::*;

        #[cfg(feature = "jwst")]
        let pool = BlobDatabase::init_memory_pool().await?;
        #[cfg(feature = "mysc")]
        let pool = Database::init_pool("jwst").await?;
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
