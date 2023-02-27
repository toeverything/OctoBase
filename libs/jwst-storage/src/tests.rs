#[cfg(test)]
use super::{
    blobs::{BlobAutoStorage, BlobModel},
    docs::{DocAutoStorage, DocsModel},
    *,
};

#[cfg(test)]
mod tests {
    use super::*;

    async fn blobs_storage_test(pool: &BlobAutoStorage) -> anyhow::Result<()> {
        // empty table
        assert_eq!(pool.count("basic").await?, 0);

        // first insert
        pool.insert("basic", "test", &[1, 2, 3, 4]).await?;
        assert_eq!(pool.count("basic").await?, 1);

        let all = pool.all("basic").await?;
        assert_eq!(
            all,
            vec![BlobModel {
                workspace: "basic".into(),
                hash: "test".into(),
                blob: vec![1, 2, 3, 4],
                length: 4,
                timestamp: all.get(0).unwrap().timestamp
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        pool.drop("basic").await?;

        pool.insert("basic", "test1", &[1, 2, 3, 4]).await?;

        let all = pool.all("basic").await?;
        assert_eq!(
            all,
            vec![BlobModel {
                workspace: "basic".into(),
                hash: "test1".into(),
                blob: vec![1, 2, 3, 4],
                length: 4,
                timestamp: all.get(0).unwrap().timestamp
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        let metadata = pool.metadata("basic", "test1").await?;

        assert_eq!(metadata.size, 4);
        assert!((metadata.last_modified.timestamp() - Utc::now().timestamp()).abs() < 2);

        pool.drop("basic").await?;

        Ok(())
    }

    async fn docs_storage_test(pool: &DocAutoStorage) -> anyhow::Result<()> {
        let conn = &pool.pool;

        pool.drop(conn, "basic").await?;

        // empty table
        assert_eq!(pool.count(conn, "basic").await?, 0);

        // first insert
        pool.insert(conn, "basic", &[1, 2, 3, 4]).await?;
        pool.insert(conn, "basic", &[2, 2, 3, 4]).await?;
        assert_eq!(pool.count(conn, "basic").await?, 2);

        // second insert
        pool.replace_with(conn, "basic", vec![3, 2, 3, 4]).await?;

        let all = pool.all(conn, "basic").await?;
        assert_eq!(
            all,
            vec![DocsModel {
                id: all.get(0).unwrap().id,
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![3, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count(conn, "basic").await?, 1);

        pool.drop(conn, "basic").await?;

        pool.insert(conn, "basic", &[1, 2, 3, 4]).await?;

        let all = pool.all(conn, "basic").await?;
        assert_eq!(
            all,
            vec![DocsModel {
                id: all.get(0).unwrap().id,
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count(conn, "basic").await?, 1);

        Ok(())
    }

    async fn full_migration_test(pool: &DocAutoStorage) -> anyhow::Result<()> {
        let final_bytes: Vec<u8> = (0..1024 * 100).map(|_| rand::random::<u8>()).collect();
        for i in 0..=50 {
            let random_bytes: Vec<u8> = if i == 50 {
                final_bytes.clone()
            } else {
                (0..1024 * 100).map(|_| rand::random::<u8>()).collect()
            };
            let (r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15) = tokio::join!(
                pool.write_full_update("full_migration_1".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_2".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_3".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_4".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_5".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_6".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_7".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_8".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_9".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_10".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_11".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_12".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_13".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_14".to_owned(), random_bytes.clone()),
                pool.write_full_update("full_migration_15".to_owned(), random_bytes.clone())
            );
            r1?;
            r2?;
            r3?;
            r4?;
            r5?;
            r6?;
            r7?;
            r8?;
            r9?;
            r10?;
            r11?;
            r12?;
            r13?;
            r14?;
            r15?;
        }

        assert_eq!(
            pool.all(&pool.pool, "full_migration_1")
                .await?
                .into_iter()
                .map(|d| d.blob)
                .collect::<Vec<_>>(),
            vec![final_bytes.clone()]
        );

        assert_eq!(
            pool.all(&pool.pool, "full_migration_2")
                .await?
                .into_iter()
                .map(|d| d.blob)
                .collect::<Vec<_>>(),
            vec![final_bytes]
        );

        Ok(())
    }

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
