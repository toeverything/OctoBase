use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{
    stream::{iter, StreamExt},
    Stream,
};
use jwst_core::{Base64Engine, BlobMetadata, URL_SAFE_ENGINE};
use sea_orm::FromQueryResult;
use sha2::{Digest, Sha256};

pub async fn get_hash(stream: impl Stream<Item = Bytes> + Send) -> (String, Vec<u8>) {
    let mut hasher = Sha256::new();

    let buffer = stream
        .flat_map(|buffer| {
            hasher.update(&buffer);
            iter(buffer)
        })
        .collect()
        .await;

    let hash = URL_SAFE_ENGINE.encode(hasher.finalize());
    (hash, buffer)
}

#[derive(FromQueryResult)]
pub(super) struct InternalBlobMetadata {
    pub(super) size: i64,
    pub(super) created_at: DateTime<Utc>,
}

impl From<InternalBlobMetadata> for BlobMetadata {
    fn from(val: InternalBlobMetadata) -> Self {
        BlobMetadata {
            content_type: "application/octet-stream".into(),
            last_modified: val.created_at.naive_local(),
            size: val.size,
        }
    }
}
