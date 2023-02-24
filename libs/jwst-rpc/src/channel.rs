use axum::extract::ws::Message;
use dashmap::DashMap;
use nanoid::nanoid;
use tokio::sync::mpsc::Sender;

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct ChannelItem {
    pub workspace: String,
    pub identifier: String,
    pub(crate) uuid: String,
}

impl ChannelItem {
    pub fn new<W, I>(workspace: W, identifier: I) -> Self
    where
        W: AsRef<str>,
        I: AsRef<str>,
    {
        Self {
            workspace: workspace.as_ref().into(),
            identifier: identifier.as_ref().into(),
            uuid: nanoid!(10),
        }
    }
}

pub type Channels = DashMap<ChannelItem, Sender<Message>>;
