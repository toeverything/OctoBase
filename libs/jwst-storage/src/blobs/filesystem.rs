use super::*;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::{
    path::Path,
    sync::atomic::{AtomicU8, Ordering},
};
use tokio::fs::create_dir_all;

pub struct FileSystem {
    // If we are using a NFS, it would handle max parallel itself
    max_parallel: Option<Semaphore>,
    path: Box<Path>,
    temp_counter: AtomicU8,
}

impl FileSystem {
    pub async fn new(max_parallel: Option<u8>, path: Box<Path>) -> Self {
        let max_parallel = max_parallel.map(|m| Semaphore::new(m as usize));
        let meta = fs::metadata(&path).await.expect("Cannot read path");

        if !meta.is_dir() {
            panic!("{} is not a directory", path.display())
        }

        let temp_dir = path.join("temp");

        if let Err(_) = fs::metadata(&temp_dir).await {
            fs::create_dir(temp_dir.as_path())
                .await
                .expect("Create temp dir failed");
        }

        Self {
            max_parallel,
            path,
            temp_counter: AtomicU8::new(0),
        }
    }

    async fn get_parallel(&self) -> Option<SemaphorePermit> {
        if let Some(m) = &self.max_parallel {
            Some(m.acquire().await.unwrap())
        } else {
            None
        }
    }

    async fn put_file(
        &self,
        path: &Path,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> io::Result<String> {
        let _ = self.get_parallel().await;
        let temp = self.temp_counter.fetch_add(1, Ordering::AcqRel);
        let mut buf = self.path.to_path_buf();
        buf.push("temp");
        buf.push(temp.to_string());

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(buf.as_path())
            .await?;

        let mut hasher = Sha256::new();

        let mut stream = Box::pin(stream);

        while let Some(c) = stream.next().await {
            file.write_all(&c).await?;
            hasher.update(&c);
        }

        file.sync_all().await?;

        if !path.exists() {
            create_dir_all(path).await?;
        }

        let hash = base64::encode_engine(hasher.finalize(), &URL_SAFE_ENGINE);
        let path = path.join(&hash);

        fs::rename(buf, path).await?;

        Ok(hash)
    }
}

#[async_trait]
impl BlobStorage for FileSystem {
    type Read = ReaderStream<File>;

    async fn get_blob(&self, workspace: Option<String>, id: String) -> io::Result<Self::Read> {
        let _ = self.get_parallel().await;
        let file = File::open(
            workspace
                .map(|workspace| self.path.join(workspace))
                .unwrap_or_else(|| self.path.to_path_buf())
                .join(id),
        )
        .await?;

        let file = ReaderStream::new(file);

        Ok(file)
    }

    async fn get_metadata(
        &self,
        workspace: Option<String>,
        id: String,
    ) -> io::Result<BlobMetadata> {
        let meta = fs::metadata(
            workspace
                .map(|workspace| self.path.join(workspace))
                .unwrap_or_else(|| self.path.to_path_buf())
                .join(id),
        )
        .await?;

        let last_modified = meta.modified()?;
        let last_modifier: DateTime<Utc> = last_modified.into();

        Ok(BlobMetadata {
            size: meta.len(),
            last_modified: last_modifier.naive_utc(),
        })
    }

    async fn put_blob(
        &self,
        workspace: Option<String>,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> io::Result<String> {
        self.put_file(
            &workspace
                .map(|workspace| self.path.join(workspace))
                .unwrap_or_else(|| self.path.to_path_buf()),
            stream,
        )
        .await
    }

    async fn delete_blob(&self, workspace: Option<String>, id: String) -> io::Result<()> {
        let _ = self.get_parallel().await;
        fs::remove_file(
            &workspace
                .map(|workspace| self.path.join(workspace))
                .unwrap_or_else(|| self.path.to_path_buf())
                .join(id),
        )
        .await
    }

    async fn delete_workspace(&self, workspace: String) -> io::Result<()> {
        let _ = self.get_parallel().await;
        fs::remove_dir_all(self.path.join(workspace)).await
    }
}
