use std::sync::atomic::Ordering;
use std::{path::Path, sync::atomic::AtomicU8};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use futures::{stream::StreamExt, Stream};
use sha3::{Digest, Sha3_256};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Semaphore, SemaphorePermit};
use tokio::{fs, io};
use tokio_util::io::ReaderStream;

pub struct Metadata {
    pub size: u64,
    pub last_modified: NaiveDateTime,
}

#[async_trait]
pub trait BlobStorage {
    type Read: Stream + Send;

    async fn get(&self, path: impl AsRef<Path> + Send) -> io::Result<Self::Read>;
    async fn get_metedata(&self, path: impl AsRef<Path> + Send) -> io::Result<Metadata>;
    async fn put(&self, stream: impl Stream<Item = Bytes> + Send) -> io::Result<String>;
    async fn put_in_workspace(
        &self,
        workspace: i64,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> io::Result<String>;
    async fn rename(
        &self,
        from: impl AsRef<Path> + Send,
        to: impl AsRef<Path> + Send,
    ) -> io::Result<()>;
    async fn delete(&self, path: impl AsRef<Path> + Send) -> io::Result<()>;
}

pub struct LocalFs {
    // If we are using a NFS, it would handle max parallel itself
    max_parallel: Option<Semaphore>,
    path: Box<Path>,
    temp_counter: AtomicU8,
}

const URL_SAFE_ENGINE: base64::engine::fast_portable::FastPortable =
    base64::engine::fast_portable::FastPortable::from(
        &base64::alphabet::URL_SAFE,
        base64::engine::fast_portable::NO_PAD,
    );

impl LocalFs {
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

        let mut hasher = Sha3_256::new();

        let mut stream = Box::pin(stream);

        while let Some(c) = stream.next().await {
            file.write_all(&c).await?;
            hasher.update(&c);
        }

        file.sync_all().await?;

        let hash = base64::encode_engine(hasher.finalize(), &URL_SAFE_ENGINE);
        let path = path.join(&hash);

        fs::rename(buf, path).await?;

        Ok(hash)
    }
}

#[async_trait]
impl BlobStorage for LocalFs {
    type Read = ReaderStream<File>;

    async fn get(&self, path: impl AsRef<Path> + Send) -> io::Result<Self::Read> {
        let _ = self.get_parallel().await;
        let file = File::open(self.path.join(path)).await?;

        let file = ReaderStream::new(file);

        Ok(file)
    }

    async fn get_metedata(&self, path: impl AsRef<Path> + Send) -> io::Result<Metadata> {
        let meta = fs::metadata(self.path.join(path)).await?;

        let last_modified = meta.modified()?;
        let last_modifier: DateTime<Utc> = last_modified.into();

        Ok(Metadata {
            size: meta.len(),
            last_modified: last_modifier.naive_utc(),
        })
    }

    async fn put(&self, stream: impl Stream<Item = Bytes> + Send) -> io::Result<String> {
        self.put_file(&self.path, stream).await
    }

    async fn put_in_workspace(
        &self,
        workspace: i64,
        stream: impl Stream<Item = Bytes> + Send,
    ) -> io::Result<String> {
        self.put_file(&self.path.join(workspace.to_string()), stream)
            .await
    }

    async fn rename(
        &self,
        from: impl AsRef<Path> + Send,
        to: impl AsRef<Path> + Send,
    ) -> io::Result<()> {
        let _ = self.get_parallel().await;
        fs::rename(self.path.join(from), self.path.join(to)).await
    }

    async fn delete(&self, path: impl AsRef<Path> + Send) -> io::Result<()> {
        let _ = self.get_parallel().await;
        fs::remove_file(self.path.join(path)).await
    }
}
