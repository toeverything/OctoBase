use super::{store::StoreRef, *};
use crate::sync::{Arc, AtomicBool, Mutex, Ordering, RwLock};
use jwst_logger::{debug, trace};

pub type DocSubscriber = Box<dyn Fn(&[u8]) + Sync + Send + 'static>;

pub struct DocPublisher {
    store: StoreRef,
    subscribers: Arc<RwLock<Vec<Box<dyn Fn(&[u8]) + Sync + Send + 'static>>>>,
    observer: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    observing: Arc<AtomicBool>,
}

impl DocPublisher {
    pub(crate) fn new(store: StoreRef) -> Self {
        let subscribers = Arc::new(RwLock::new(Vec::<DocSubscriber>::new()));

        let publisher = Self {
            store,
            subscribers,
            observer: Arc::default(),
            observing: Arc::new(AtomicBool::new(false)),
        };

        if cfg!(not(any(feature = "bench", fuzzing, loom, miri))) {
            publisher.start();
        }

        publisher
    }

    pub fn start(&self) {
        let mut observer = self.observer.lock().unwrap();
        let observing = self.observing.clone();
        let store = self.store.clone();
        if observer.is_none() {
            let thread_subscribers = self.subscribers.clone();
            observing.store(true, Ordering::Release);
            debug!("start observing");
            let thread = std::thread::spawn(move || {
                let mut last_update = store.read().unwrap().get_state_vector();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if !observing.load(Ordering::Acquire) {
                        debug!("stop observing");
                        break;
                    }

                    let subscribers = thread_subscribers.read().unwrap();
                    if subscribers.is_empty() {
                        continue;
                    }

                    let update = store.read().unwrap().get_state_vector();
                    if update != last_update {
                        trace!(
                            "update: {:?}, last_update: {:?}, {:?}",
                            update,
                            last_update,
                            std::thread::current().id(),
                        );
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
                            cb(&binary);
                        }
                    }
                }
            });
            observer.replace(thread);
        } else {
            debug!("already observing");
        }
    }

    pub fn stop(&self) {
        let mut observer = self.observer.lock().unwrap();
        if let Some(observer) = observer.take() {
            self.observing.store(false, Ordering::Release);
            observer.join().unwrap();
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

impl Drop for DocPublisher {
    fn drop(&mut self) {
        self.stop();
    }
}
