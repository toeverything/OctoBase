mod blob;
mod doc;

use tokio::{
    fs::{self, File},
    io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
    sync::{Semaphore, SemaphorePermit},
};

pub use blob::LocalFs as BlobFsStorage;
pub use doc::LocalFs as DocFsStorage;
