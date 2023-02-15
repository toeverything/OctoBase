use super::*;
use base64::{
    alphabet::URL_SAFE,
    engine::{general_purpose::PAD, GeneralPurpose},
    Engine,
};
use bytes::Bytes;
use futures::stream::{iter, StreamExt};
use sha2::{Digest, Sha256};

const URL_SAFE_ENGINE: GeneralPurpose = GeneralPurpose::new(&URL_SAFE, PAD);

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
