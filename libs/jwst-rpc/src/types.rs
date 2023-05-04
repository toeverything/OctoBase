use thiserror::Error;
#[cfg(feature = "websocket")]
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum JwstRpcError {
    #[cfg(feature = "websocket")]
    #[error("failed to connect websocket")]
    WebsocketConnect(#[from] tungstenite::Error),
    #[error("jwst error")]
    Jwst(#[from] jwst::JwstError),
    #[cfg(feature = "websocket")]
    #[error("failed to parse url")]
    UrlParse(#[from] url::ParseError),
}

pub type JwstRpcResult<T> = Result<T, JwstRpcError>;
