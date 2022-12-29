use super::*;
use std::{
    io::{ErrorKind, SeekFrom},
    path::{Path, PathBuf},
};
use yrs::{updates::decoder::Decode, Doc, StateVector, Transact, Update};

// The structure of record in disk is basically
// length of record|record data|record count
pub struct FileSystem {
    // If we are using a NFS, it would handle max parallel itself
    max_parallel: Option<Semaphore>,
    max_update: usize,
    path: Box<Path>,
}

impl FileSystem {
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

    async fn parse_file(&self, file: &mut File) -> io::Result<Doc> {
        let mut file = BufReader::new(file);
        let doc = Doc::new();
        let mut trx = doc.transact_mut();

        loop {
            let len = file.read_u64_le().await;

            let len = match len {
                Ok(len) => len,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            let mut update = vec![0; len as usize];
            file.read_exact(&mut update).await?;

            match Update::decode_v1(&update) {
                Ok(update) => trx.apply_update(update),
                Err(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "decode error")),
            }

            file.read_u64_le().await?;
        }

        trx.commit();
        drop(trx);

        Ok(doc)
    }
}

#[async_trait]
impl DocStorage for FileSystem {
    async fn get(&self, workspace: i64) -> io::Result<Doc> {
        // let _ = self.get_parallel().await;
        // let path = self.get_path(workspace);

        // let mut file = File::open(path).await?;

        // self.parse_file(&mut file).await
        todo!("future created by async block is not `Send`")
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_doc(&self, workspace: i64, doc: &Doc) -> io::Result<()> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await?;

        let data = [];
        todo!("doc.encode_state_as_update_v1(&StateVector::default())");

        let data = Self::make_record(&data, 0);

        file.write_all(&data).await?;

        file.sync_all().await
    }

    /// This function is not atomic -- please provide external lock mechanism
    async fn write_update(&self, workspace: i64, data: &[u8]) -> io::Result<bool> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);

        let mut file = fs::OpenOptions::new()
            .read(true)
            .append(true)
            .open(&path)
            .await?;
        let size = file.metadata().await?.len();

        file.seek(SeekFrom::Start(size - 8)).await?;

        let count = file.read_u64_le().await? + 1;

        if count as usize > self.max_update {
            return Ok(false);
        }

        let data = Self::make_record(data, count);

        file.write_all(&data).await?;
        file.sync_all().await?;

        Ok(true)
    }

    async fn delete(&self, workspace: i64) -> io::Result<()> {
        let _ = self.get_parallel().await;
        let path = self.get_path(workspace);
        fs::remove_file(path).await
    }
}
