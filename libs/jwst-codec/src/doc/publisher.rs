use super::{store::StoreRef, *};
use crate::sync::{Arc, RwLock};

pub type DocSubscriber = Box<dyn Fn(&[u8]) + Sync + Send + 'static>;

pub struct DocPublisher {
    subscribers: Arc<RwLock<Vec<Box<dyn Fn(&[u8]) + Sync + Send + 'static>>>>,
    _observer: Arc<Option<std::thread::JoinHandle<()>>>,
}

impl DocPublisher {
    pub(crate) fn new(store: StoreRef) -> Self {
        let subscribers = Arc::new(RwLock::new(Vec::<DocSubscriber>::new()));

        // skip observe thread create in test and benchmark
        if cfg!(any(feature = "bench", fuzzing, loom, miri)) {
            Self {
                subscribers,
                _observer: Arc::new(None),
            }
        } else {
            let thread_subscribers = subscribers.clone();
            let thread = std::thread::spawn(move || {
                let mut last_update = store.read().unwrap().get_state_vector();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));

                    let subscribers = thread_subscribers.read().unwrap();
                    if subscribers.is_empty() {
                        continue;
                    }

                    let update = store.read().unwrap().get_state_vector();
                    if update != last_update {
                        let mut encoder = RawEncoder::default();
                        if let Err(e) = store
                            .read()
                            .unwrap()
                            .encode_with_state_vector(&last_update, &mut encoder)
                        {
                            warn!("Failed to encode document: {}", e);
                            continue;
                        };

                        last_update = update;

                        let binary = encoder.into_inner();

                        for cb in subscribers.iter() {
                            cb(&binary)
                        }
                    }
                }
            });
            Self {
                subscribers,
                _observer: Arc::new(Some(thread)),
            }
        }
    }

    pub(crate) fn subscribe(&self, subscriber: impl Fn(&[u8]) + Send + Sync + 'static) {
        self.subscribers.write().unwrap().push(Box::new(subscriber));
    }

    pub(crate) fn unsubscribe_all(&self) {
        self.subscribers.write().unwrap().clear();
    }
}

impl std::fmt::Debug for DocPublisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocPublisher").finish()
    }
}
