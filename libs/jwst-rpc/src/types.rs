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
    #[allow(dead_code)]
    #[error("failed to encode sync message")]
    ProtocolEncode(std::io::Error),
    #[error("failed to decode sync message")]
    ProtocolDecode(#[from] nom::Err<nom::error::Error<usize>>),
    #[cfg(feature = "websocket")]
    #[error("failed to parse url")]
    UrlParse(#[from] url::ParseError),
}

pub type JwstRpcResult<T> = Result<T, JwstRpcError>;
