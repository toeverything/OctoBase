use jwst_codec::JwstCodecError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwstStorageError {
    #[error("failed to init sync thread")]
    SyncThread(std::io::Error),
    #[error("failed to execute blocking thread")]
    BlockingThread(tokio::task::JoinError),
    #[error("failed to create data directory")]
    CreateDataFolder(std::io::Error),
    #[error("db manipulate error: {0}")]
    Crud(String),
    #[error("db error: {0:?}")]
    Db(#[from] sea_orm::DbErr),
    #[error("doc codec error")]
    DocJwstCodec(#[from] JwstCodecError),
    #[error("merge thread panic")]
    DocMerge(tokio::task::JoinError),
    #[error("workspace {0} not found")]
    WorkspaceNotFound(String),
    #[error("workspace {0} exists")]
    WorkspaceExists(String),
    #[error("jwst error")]
    Jwst(#[from] jwst_core::JwstError),
    #[error("failed to process blob")]
    JwstBlob(#[from] crate::storage::blobs::JwstBlobError),
    #[cfg(feature = "bucket")]
    #[error("bucket error")]
    JwstBucket(#[from] opendal::Error),
    #[cfg(feature = "bucket")]
    #[error("env variables read error")]
    DotEnvy(#[from] dotenvy::Error),
}

pub type JwstStorageResult<T = ()> = Result<T, JwstStorageError>;
