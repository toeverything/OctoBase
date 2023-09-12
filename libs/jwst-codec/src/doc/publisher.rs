use std::{
    thread::{current, sleep, spawn},
    time::Duration,
};

use jwst_logger::{debug, trace};

use super::{history::StoreHistory, store::StoreRef, *};
use crate::sync::{Arc, AtomicBool, Mutex, Ordering, RwLock};

pub type DocSubscriber = Box<dyn Fn(&[u8], &[History]) + Sync + Send + 'static>;

const OBSERVE_INTERVAL: u64 = 100;

pub struct DocPublisher {
    store: StoreRef,
    history: StoreHistory,
    subscribers: Arc<RwLock<Vec<DocSubscriber>>>,
    observer: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    observing: Arc<AtomicBool>,
}

impl DocPublisher {
    pub(crate) fn new(store: StoreRef) -> Self {
        let subscribers = Arc::new(RwLock::new(Vec::<DocSubscriber>::new()));
        let history = StoreHistory::new(&store);
        history.resolve();

        let publisher = Self {
            store,
            history,
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
        let history = self.history.clone();
        if observer.is_none() {
            let thread_subscribers = self.subscribers.clone();
            observing.store(true, Ordering::Release);
            debug!("start observing");
            let thread = spawn(move || {
                let mut last_update = store.read().unwrap().get_state_vector();
                loop {
                    sleep(Duration::from_millis(OBSERVE_INTERVAL));
                    if !observing.load(Ordering::Acquire) {
                        debug!("stop observing");
                        break;
                    }

                    let subscribers = thread_subscribers.read().unwrap();
                    if subscribers.is_empty() {
                        continue;
                    }

                    let store = store.read().unwrap();

                    let update = store.get_state_vector();
                    if update != last_update {
                        trace!(
                            "update: {:?}, last_update: {:?}, {:?}",
                            update,
                            last_update,
                            current().id(),
                        );

                        history.resolve_with_store(&store);
                        let (binary, history) = match store.diff_state_vector(&last_update) {
                            Ok(update) => {
                                drop(store);

                                let history = history.parse_update(&update, 0);

                                let mut encoder = RawEncoder::default();
                                if let Err(e) = update.write(&mut encoder) {
                                    warn!("Failed to encode document: {}", e);
                                    continue;
                                }
                                (encoder.into_inner(), history)
                            }
                            Err(e) => {
                                warn!("Failed to diff document: {}", e);
                                continue;
                            }
                        };

                        last_update = update;

                        for cb in subscribers.iter() {
                            cb(&binary, &history);
                        }
                    } else {
                        drop(store);
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

    pub(crate) fn subscribe(&self, subscriber: impl Fn(&[u8], &[History]) + Send + Sync + 'static) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_update_history() {
        let doc = Doc::default();

        // update: 24
        // history change by (1, 0) at test.key1: val1
        // update: 43
        // history change by (1, 1) at test.key2: val2
        // history change by (1, 2) at test.key3: val3
        // update: 40
        // history change by (1, 3) at array.0: val1
        // history change by (1, 4) at array.1: val2
        // history change by (1, 5) at array.2: val3
        doc.subscribe(|u, history| {
            println!("update: {}", u.len());
            for h in history {
                println!("history change by {} at {}: {}", h.id, h.parent.join("."), h.content);
            }
        });

        let mut map = doc.get_or_create_map("test").unwrap();
        map.insert("key1", "val1").unwrap();

        sleep(Duration::from_secs(1));

        map.insert("key2", "val2").unwrap();
        map.insert("key3", "val3").unwrap();
        sleep(Duration::from_secs(1));

        let mut array = doc.get_or_create_array("array").unwrap();
        array.push("val1").unwrap();
        array.push("val2").unwrap();
        array.push("val3").unwrap();

        sleep(Duration::from_secs(1));
    }
}
