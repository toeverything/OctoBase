mod broadcast;
#[cfg(feature = "websocket")]
mod client;
mod connector;
mod context;
mod handler;
mod types;
mod utils;

#[cfg(feature = "websocket")]
pub use client::{get_collaborating_workspace, get_workspace, start_sync_thread};
#[cfg(feature = "websocket")]
pub use connector::socket_connector;

pub use broadcast::{BroadcastChannels, BroadcastType};
pub use connector::memory_connector;
pub use context::RpcContextImpl;
pub use handler::handle_connector;
pub use utils::{connect_memory_workspace, MinimumServerContext};

use jwst::{debug, error, info, trace};
use std::{collections::hash_map::Entry, sync::Arc, time::Instant};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};

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
