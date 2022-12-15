use std::io::{ErrorKind, SeekFrom};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use fs4::tokio::AsyncFileExt;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::sync::{Semaphore, SemaphorePermit};
use tokio::task::spawn_blocking;
use tokio::{fs, io};
use yrs::updates::decoder::Decode;
use yrs::{Doc, StateVector, Update};

#[async_trait]
pub trait DocStorage {
    async fn get(&self, workspace_id: i64) -> io::Result<Vec<Box<[u8]>>>;
    async fn create(&self, workspace_id: i64, doc: &Doc) -> io::Result<()>;
    async fn write_update(&self, workspace: i64, data: &[u8]) -> io::Result<()>;
    async fn delete(&self, workspace_id: i64) -> io::Result<()>;
}

fn migrate_update(updates: &[Box<[u8]>], doc: Doc) -> Doc {
    let mut trx = doc.transact();
    for update in updates {
        match Update::decode_v1(update) {
            Ok(update) => {
                if let Err(e) = catch_unwind(AssertUnwindSafe(|| trx.apply_update(update))) {
                    // warn!("update {} merge failed, skip it: {:?}", id, e);
                }
            }
            Err(_) => {} // Err(err) => info!("failed to decode update: {:?}", err),
        }
    }
    trx.commit();

    doc
}

// The structure of record in disk is basically
// length of record|record data|record count
pub struct LocalFs {
    // If we are using a NFS, it would handle max parallel itself
    max_parallel: Option<Semaphore>,
    max_update: usize,
    path: Box<Path>,
}

impl LocalFs {
    pub async fn new(max_parallel: Option<u8>, max_update: usize, path: Box<Path>) -> Self {
        let max_parallel = max_parallel.map(|m| Semaphore::new(m as usize));
        let meta = fs::metadata(&path).await.expect("Cannot read path");

        if !meta.is_dir() {
            panic!("{} is not a directory", path.display())
        }

        Self {
            max_parallel,
            max_update,
            path,
        }
    }

    async fn get_parallel(&self) -> Option<SemaphorePermit> {
        if let Some(m) = &self.max_parallel {
            Some(m.acquire().await.unwrap())
        } else {
            None
        }
    }

    fn get_path(&self, workspace: i64) -> PathBuf {
        self.path.join(format!("{}.affine", workspace))
    }

    fn make_record(data: &[u8], count: u64) -> Vec<u8> {
        let mut buffer = (data.len() as u64).to_le_bytes().to_vec();
        buffer.extend(data);
        buffer.extend(count.to_le_bytes());
        buffer
    }

    async fn parse_file(&self, file: &mut File) -> io::Result<Vec<Box<[u8]>>> {
        let mut file = BufReader::new(file);
        let mut res = Vec::new();

        loop {
            let len = file.read_u64_le().await;

            let len = match len {
                Ok(len) => len,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            let mut buf = vec![0; len as usize];
            file.read_exact(&mut buf).await?;

            res.push(buf.into_boxed_slice());

            file.read_u64_le().await?;
        }

        Ok(res)
    }
}

#[async_trait]
impl DocStorage for LocalFs {
    async fn get(&self, workspace: i64) -> io::Result<Vec<Box<[u8]>>> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);

        let mut file = File::open(path).await?;

        self.parse_file(&mut file).await
    }

    async fn create(&self, workspace: i64, doc: &Doc) -> io::Result<()> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .await?;

        let data = doc.encode_state_as_update_v1(&StateVector::default());

        let data = Self::make_record(&data, 0);

        file.write_all(&data).await?;

        file.sync_all().await
    }

    async fn write_update(&self, workspace: i64, data: &[u8]) -> io::Result<()> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);

        let mut file = fs::OpenOptions::new().append(true).open(&path).await?;

        let size = file.metadata().await?.len();

        file.seek(SeekFrom::Start(size - 8)).await?;

        // There maybe potential race condition but it doesn't matter
        // As we doesn't need exact count
        let count = file.read_u64_le().await? + 1;

        if count as usize > self.max_update {
            let mut file = spawn_blocking(move || file.lock_exclusive().map(|_| file)).await??;

            let mut file_data = self.parse_file(&mut file).await?;

            file_data.push(data.into());

            let doc = migrate_update(&file_data, Doc::default());

            let data = doc.encode_state_as_update_v1(&StateVector::default());

            let data = Self::make_record(&data, 0);

            file.set_len(0).await?;

            file.write_all(&data).await?;

            file.sync_all().await?;

            spawn_blocking(move || file.unlock()).await??;
        } else {
            let data = Self::make_record(data, count);

            file.write_all(&data).await?;
            file.sync_all().await?;
        }

        Ok(())
    }

    async fn delete(&self, workspace: i64) -> io::Result<()> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);
        fs::remove_file(path).await
    }
}
