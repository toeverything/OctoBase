#[cfg(test)]
use super::{
    blobs::blobs_storage_test,
    docs::{docs_storage_test, full_migration_test},
    *,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_storage_test() -> anyhow::Result<()> {
        let storage = JwstStorage::new("sqlite::memory:").await?;

        blobs_storage_test(storage.blobs()).await?;
        docs_storage_test(storage.docs()).await?;

        Ok(())
    }

    #[ignore = "need postgres server"]
    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_storage_test() -> anyhow::Result<()> {
        let db = "postgresql://affine:affine@localhost:5432/affine_binary";
        let storage = JwstStorage::new(db).await?;
        let (r1, r2, r3) = tokio::join!(
            blobs_storage_test(storage.blobs()),
            docs_storage_test(storage.docs()),
            full_migration_test(storage.docs()),
        );

        r1?;
        r2?;
        r3?;

        Ok(())
    }
}
