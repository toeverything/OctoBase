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
        pool.drop("basic").await?;

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
            vec![DocsModel {
                id: all.get(0).unwrap().id,
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
            vec![DocsModel {
                id: all.get(0).unwrap().id,
                workspace: "basic".into(),
                timestamp: all.get(0).unwrap().timestamp,
                blob: vec![1, 2, 3, 4]
            }]
        );
        assert_eq!(pool.count("basic").await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn sqlite_storage_test() -> anyhow::Result<()> {
        let storage = JwstStorage::new("sqlite::memory:").await?;

        blobs_storage_test(storage.blobs()).await?;
        docs_storage_test(storage.docs()).await?;

        Ok(())
    }

    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn postgres_storage_test() -> anyhow::Result<()> {
        let db = "postgresql://affine:affine@localhost:5432/affine_binary";
        let storage = JwstStorage::new(db).await?;

        blobs_storage_test(storage.blobs()).await?;
        docs_storage_test(storage.docs()).await?;

        Ok(())
    }
}
