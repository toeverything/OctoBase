#[forbid(unsafe_code)]
mod broadcast;
mod client;
mod connector;
mod context;
mod handler;
mod types;
mod utils;

use std::{collections::hash_map::Entry, sync::Arc, time::Instant};

pub use broadcast::{BroadcastChannels, BroadcastType};
#[cfg(feature = "webrtc")]
pub use client::start_webrtc_client_sync;
#[cfg(feature = "websocket")]
pub use client::start_websocket_client_sync;
pub use client::CachedLastSynced;
pub use connector::memory_connector;
#[cfg(feature = "webrtc")]
pub use connector::webrtc_datachannel_client_begin;
#[cfg(feature = "webrtc")]
pub use connector::webrtc_datachannel_client_commit;
#[cfg(feature = "webrtc")]
pub use connector::webrtc_datachannel_server_connector;
#[cfg(feature = "websocket")]
pub use connector::{axum_socket_connector, tungstenite_socket_connector};
pub use context::RpcContextImpl;
pub use handler::handle_connector;
use jwst_core::{debug, error, info, trace, warn};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};
pub use utils::{connect_memory_workspace, MinimumServerContext};
#[cfg(feature = "webrtc")]
pub use webrtcrs::peer_connection::sdp::session_description::RTCSessionDescription;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum SyncState {
    #[default]
    Offline,
    Connected,
    Finished,
    Error(String),
}

#[derive(Debug)]
pub enum Message {
    Binary(Vec<u8>),
    Close,
    Ping,
}
