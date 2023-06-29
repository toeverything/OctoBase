use jwst_codec::JwstCodecError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwstStorageError {
    #[error("failed to create data directory")]
    CreateDataFolder(std::io::Error),
    #[error("db manipulate error: {0}")]
    Crud(String),
    #[error("db error")]
    Db(#[from] sea_orm::DbErr),
    #[error("failed to process doc")]
    DocCodec(#[from] lib0::error::Error),
    #[error("doc codec error")]
    DocJwstCodec(#[from] JwstCodecError),
    #[error("merge thread panic")]
    DocMerge(tokio::task::JoinError),
    #[error("workspace {0} not found")]
    WorkspaceNotFound(String),
    #[error("jwst error")]
    Jwst(#[from] jwst::JwstError),
    #[error("failed to process blob")]
    JwstBlob(#[from] crate::storage::blobs::JwstBlobError),
    #[error("bucket error")]
    JwstBucket(#[from] opendal::Error),
    #[error("env variables read error")]
    DotEnvy(#[from] dotenvy::Error),
}

pub type JwstStorageResult<T> = Result<T, JwstStorageError>;
