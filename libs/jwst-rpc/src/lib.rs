mod broadcast;
mod client;
mod connector;
mod context;
mod handler;
mod protocol;
mod types;
mod utils;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use client::{get_collaborating_workspace, get_workspace, start_sync_thread};
pub use connector::{memory_connector, socket_connector};
pub use context::RpcContextImpl;
pub use handler::handle_connector;
pub use protocol::{read_sync_message, write_sync_message, SyncMessage};
pub use utils::{connect_memory_workspace, MinimumServerContext};

use jwst::{debug, error, info, trace, warn};
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};
use types::{JwstRpcError, JwstRpcResult};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SyncState {
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
