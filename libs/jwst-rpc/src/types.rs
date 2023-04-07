use thiserror::Error;
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum JwstRpcError {
    #[error("failed to connect websocket")]
    WebsocketConnect(#[from] tungstenite::Error),
    #[error("jwst error")]
    Jwst(#[from] jwst::JwstError),
    #[error("failed to parse url")]
    UrlParse(#[from] url::ParseError),
}

pub type JwstRPCResult<T> = Result<T, JwstRpcError>;
