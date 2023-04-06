use thiserror::Error;
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum JwstRPCError {
    #[error(transparent)]
    BoxedError(#[from] anyhow::Error),
    #[error("websocket connect error")]
    WebsocketConnectError(#[from] tungstenite::Error),
    #[error("jwst error")]
    JwstError(#[from] jwst::JwstError),
}

pub type JwstRPCResult<T> = Result<T, JwstRPCError>;
