use tokio::sync::Mutex;
use yrs::{types::Events, Subscription};

pub struct BlockSubscription(Mutex<Subscription<Events>>);

unsafe impl Send for BlockSubscription {}
unsafe impl Sync for BlockSubscription {}

impl From<Subscription<Events>> for BlockSubscription {
    fn from(s: Subscription<Events>) -> Self {
        Self(Mutex::new(s))
    }
}
