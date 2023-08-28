use thiserror::Error;
#[cfg(feature = "websocket")]
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum JwstRpcError {
    #[cfg(feature = "websocket")]
    #[error("failed to connect websocket: {0}")]
    WebsocketConnect(#[from] tungstenite::Error),
    #[error("jwst error")]
    Jwst(#[from] jwst_core::JwstError),
    #[allow(dead_code)]
    #[error("failed to encode sync message")]
    ProtocolEncode(std::io::Error),
    #[cfg(feature = "websocket")]
    #[error("failed to parse url")]
    UrlParse(#[from] url::ParseError),
}

pub type JwstRpcResult<T> = Result<T, JwstRpcError>;
