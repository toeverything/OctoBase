use super::*;
use bytes::Bytes;
use futures::stream::{iter, StreamExt};
use jwst::{Base64Engine, URL_SAFE_ENGINE};
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
