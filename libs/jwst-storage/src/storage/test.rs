#[cfg(test)]
use super::{
    blobs::blobs_storage_test,
    docs::{docs_storage_partial_test, docs_storage_test},
    *,
};

#[tokio::test]
async fn sqlite_storage_test() -> anyhow::Result<()> {
    let storage = JwstStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB).await?;

    blobs_storage_test(&storage.blobs().get_blob_db().unwrap()).await?;
    docs_storage_test(&storage.docs().0).await?;
    docs_storage_partial_test(&storage.docs().0).await?;

    Ok(())
}

#[ignore = "need postgres server"]
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_storage_test() -> anyhow::Result<()> {
    use test::docs::full_migration_stress_test;

    let db = "postgresql://affine:affine@localhost:5432/affine_binary";
    let storage = JwstStorage::new_with_migration(db, BlobStorageType::DB).await?;
    let blob_db = storage.blobs().get_blob_db().unwrap();
    let (r1, r2, r3, r4) = tokio::join!(
        blobs_storage_test(&blob_db),
        docs_storage_test(&storage.docs().0),
        docs_storage_partial_test(&storage.docs().0),
        full_migration_stress_test(&storage.docs().0),
    );

    r1?;
    r2?;
    r3?;
    r4?;

    Ok(())
}
