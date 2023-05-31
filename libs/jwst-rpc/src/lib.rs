#[forbid(unsafe_code)]
mod broadcast;
#[cfg(feature = "websocket")]
mod client;
#[cfg(feature = "webrtc")]
mod client_webrtc;
mod connector;
mod context;
mod handler;
mod types;
mod utils;

#[cfg(feature = "websocket")]
pub use client::start_client_sync;
#[cfg(feature = "webrtc")]
pub use client_webrtc::start_webrtc_client_sync;
#[cfg(feature = "webrtc")]
pub use connector::webrtc_datachannel_client_begin;
#[cfg(feature = "webrtc")]
pub use connector::webrtc_datachannel_client_commit;
#[cfg(feature = "webrtc")]
pub use connector::webrtc_datachannel_server_connector;
#[cfg(feature = "websocket")]
pub use connector::{axum_socket_connector, tungstenite_socket_connector};
#[cfg(feature = "webrtc")]
pub use webrtcrs::peer_connection::sdp::session_description::RTCSessionDescription;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use connector::memory_connector;
pub use context::RpcContextImpl;
pub use handler::handle_connector;
pub use utils::{connect_memory_workspace, MinimumServerContext};

use jwst::{debug, error, info, trace, warn};
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum SyncState {
    #[default]
    Offline,
    Initialized,
    Syncing,
    Finished,
    Error(String),
}

#[derive(Debug)]
pub enum Message {
    Binary(Vec<u8>),
    Close,
    Ping,
}
