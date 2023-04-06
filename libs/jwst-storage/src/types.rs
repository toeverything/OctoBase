use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwstStorageError {
    #[error("db error")]
    DbError(#[from] sea_orm::DbErr),
    #[error("db manipulate error: {0}")]
    CRUDError(String),
    #[error("workspace {0} not found")]
    WorkspaceNotFound(String),
    #[error("jwst error")]
    JwstError(#[from] jwst::JwstError),
    #[error("jwst blob error")]
    JwstBlobError(#[from] crate::storage::blobs::JwstBlobError),
    #[error("yrs lib0 error")]
    YrsLib0Error(#[from] lib0::error::Error),
    #[error("tokio error")]
    TokioError(#[from] tokio::task::JoinError),
    #[error("io error")]
    IOError(#[from] std::io::Error),
}

pub type JwstStorageResult<T> = Result<T, JwstStorageError>;