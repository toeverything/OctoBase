use jwst_core::{Base64Engine, JwstResult, URL_SAFE_ENGINE};
use opendal::{services::S3, Operator};
use sha2::{Digest, Sha256};

use super::*;

pub struct MixedBucketDBParam {
    pub(crate) access_key: String,
    pub(crate) secret_access_key: String,
    pub(crate) endpoint: String,
    pub(crate) bucket: Option<String>,
    pub(crate) root: Option<String>,
}

impl MixedBucketDBParam {
    pub fn new_from_env() -> JwstResult<Self, JwstStorageError> {
        Ok(MixedBucketDBParam {
            access_key: dotenvy::var("BUCKET_ACCESS_TOKEN")?,
            secret_access_key: dotenvy::var("BUCKET_SECRET_TOKEN")?,
            endpoint: dotenvy::var("BUCKET_ENDPOINT")?,
            bucket: dotenvy::var("BUCKET_NAME").ok(),
            root: dotenvy::var("BUCKET_ROOT").ok(),
        })
    }

    pub fn new(
        access_key: String,
        secret_access_key: String,
        endpoint: String,
        bucket: Option<String>,
        root: Option<String>,
    ) -> Self {
        MixedBucketDBParam {
            access_key,
            secret_access_key,
            endpoint,
            bucket,
            root,
        }
    }
}

impl TryFrom<MixedBucketDBParam> for BucketStorage {
    type Error = JwstStorageError;

    fn try_from(value: MixedBucketDBParam) -> Result<Self, Self::Error> {
        let mut builder = BucketStorageBuilder::new();
        builder = builder.access_key(&value.access_key);
        builder = builder.secret_access_key(&value.secret_access_key);
        builder = builder.endpoint(&value.endpoint);
        builder = builder.bucket(&value.bucket.unwrap_or("__default_bucket__".to_string()));
        builder = builder.root(&value.root.unwrap_or("__default_root__".to_string()));
        builder.build()
    }
}

#[derive(Default)]
pub struct BucketStorageBuilder {
    access_key: String,
    secret_access_key: String,
    endpoint: String,
    bucket: String,
    root: String,
}

impl BucketStorageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn access_key(mut self, access_key: &str) -> Self {
        self.access_key = access_key.to_string();
        self
    }

    pub fn secret_access_key(mut self, secret_access_key: &str) -> Self {
        self.secret_access_key = secret_access_key.to_string();
        self
    }

    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.to_string();
        self
    }

    pub fn bucket(mut self, bucket: &str) -> Self {
        self.bucket = bucket.to_string();
        self
    }

    pub fn root(mut self, root: &str) -> Self {
        self.root = root.to_string();
        self
    }

    pub fn build(self) -> JwstStorageResult<BucketStorage> {
        let mut builder = S3::default();

        builder.bucket(self.bucket.as_str());
        builder.root(self.root.as_str());
        builder.endpoint(self.endpoint.as_str());
        builder.access_key_id(self.access_key.as_str());
        builder.secret_access_key(self.secret_access_key.as_str());

        Ok(BucketStorage {
            op: Operator::new(builder)?.finish(),
        })
    }
}

/// Calculate sha256 hash for given blob
pub fn calculate_hash(blob: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(blob);
    URL_SAFE_ENGINE.encode(hasher.finalize())
}
