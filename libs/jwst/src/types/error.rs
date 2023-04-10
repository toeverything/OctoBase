use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwstError {
    // #[error("database error")]
    // Database(#[from] DbErr),
    #[error(transparent)]
    BoxedError(#[from] anyhow::Error),
    #[error(transparent)]
    StorageError(anyhow::Error),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("doc codec error")]
    DocCodec(#[from] lib0::error::Error),
    #[error("doc transaction error")]
    DocTransaction(String),
    #[error("workspace {0} not initialized")]
    WorkspaceNotInitialized(String),
    // version metadata
    #[error("workspace {0} has no version")]
    VersionNotFound(String),
    // page metadata
    #[error("workspace {0} has no page tree")]
    PageTreeNotFound(String),
    #[error("page item {0} not found")]
    PageItemNotFound(String),
    #[error("failed to get state vector")]
    SyncInitTransaction,
    #[error("y_sync awareness error")]
    YSyncAwarenessErr(#[from] y_sync::awareness::Error),
}

pub type JwstResult<T, E = JwstError> = Result<T, E>;