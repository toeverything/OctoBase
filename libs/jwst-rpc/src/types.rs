use thiserror::Error;
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum JwstRpcError {
    #[error("failed to connect websocket")]
    WebsocketConnect(#[from] tungstenite::Error),
    #[error("jwst error")]
    Jwst(#[from] jwst::JwstError),
    #[error("failed to encode sync message")]
    ProtocolEncode,
    #[error("failed to decode sync message")]
    ProtocolDecode(#[from] nom::Err<nom::error::Error<usize>>),
    #[error("failed to parse url")]
    UrlParse(#[from] url::ParseError),
}

pub type JwstRpcResult<T> = Result<T, JwstRpcError>;
