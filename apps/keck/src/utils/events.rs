use super::*;

use time::OffsetDateTime;
use tokio::sync::Mutex;
use utoipa::ToSchema;
use yrs::{types::Events, Subscription};

pub struct BlockSubscription(Mutex<Subscription<Events>>);

unsafe impl Send for BlockSubscription {}
unsafe impl Sync for BlockSubscription {}

impl From<Subscription<Events>> for BlockSubscription {
    fn from(s: Subscription<Events>) -> Self {
        Self(Mutex::new(s))
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlockHistory {
    pub timestamp: i64,
    path: String,
}

impl BlockHistory {
    pub fn new(path: String) -> Self {
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();
        Self { timestamp, path }
    }
}
