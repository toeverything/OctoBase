use governor::{
    clock::{QuantaClock, QuantaInstant},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{num::NonZeroU32, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, RwLock, RwLockReadGuard, RwLockWriteGuard, Semaphore};
use url::Url;

pub enum BucketLocker<'a> {
    Semaphore(OwnedSemaphorePermit),
    ReadLock(RwLockReadGuard<'a, ()>),
    WriteLock(RwLockWriteGuard<'a, ()>),
}

enum BucketLock {
    Semaphore(Arc<Semaphore>),
    RwLock(Arc<RwLock<()>>),
}

pub struct Bucket {
    bucket: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>,
    lock: BucketLock,
}

impl Bucket {
    fn new(bucket_size: u32, semaphore_size: usize) -> Self {
        let bucket_size =
            NonZeroU32::new(bucket_size).unwrap_or(unsafe { NonZeroU32::new_unchecked(1) });

        Self {
            bucket: Arc::new(RateLimiter::direct(
                Quota::per_second(bucket_size).allow_burst(bucket_size),
            )),
            lock: if semaphore_size > 1 {
                BucketLock::Semaphore(Arc::new(Semaphore::new(semaphore_size)))
            } else {
                // use for sqlite
                BucketLock::RwLock(Arc::default())
            },
        }
    }

    pub async fn read(&self) -> BucketLocker {
        self.bucket.until_ready().await;
        match &self.lock {
            BucketLock::RwLock(lock) => BucketLocker::ReadLock(lock.read().await),
            BucketLock::Semaphore(semaphore) => {
                BucketLocker::Semaphore(semaphore.clone().acquire_owned().await.unwrap())
            }
        }
    }

    pub async fn write(&self) -> BucketLocker {
        self.bucket.until_ready().await;
        match &self.lock {
            BucketLock::RwLock(lock) => BucketLocker::WriteLock(lock.write().await),
            BucketLock::Semaphore(semaphore) => {
                BucketLocker::Semaphore(semaphore.clone().acquire_owned().await.unwrap())
            }
        }
    }
}

#[inline]
pub fn is_sqlite(database: &str) -> bool {
    Url::parse(database)
        .map(|u| u.scheme() == "sqlite")
        .unwrap_or(false)
}

#[inline]
pub fn get_bucket(single_thread: bool) -> Arc<Bucket> {
    // sqlite only support 1 writer at a time
    // we use semaphore to limit the number of concurrent writers
    Arc::new(Bucket::new(
        if single_thread { 10 } else { 25 },
        if single_thread { 1 } else { 5 },
    ))
}
