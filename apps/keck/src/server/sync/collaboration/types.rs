use super::*;
use libp2p::{gossipsub::error::SubscriptionError, TransportError};
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CollaborationError {
    #[error("failed to init broadcast server: {0}")]
    InitError(#[from] io::Error),
    #[error("failed to subscribe to topic: {0}")]
    SubscribeError(#[from] SubscriptionError),
    #[error("failed to listen: {0}")]
    ListenError(#[from] TransportError<io::Error>),
}

pub type CollaborationResult<T> = Result<T, CollaborationError>;
