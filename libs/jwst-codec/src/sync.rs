#[cfg(loom)]
pub(crate) use loom::{
    sync::{
        atomic::{AtomicU8, AtomicUsize, Ordering},
        Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
    thread,
};

#[cfg(not(loom))]
pub(crate) use std::sync::{
    atomic::{AtomicU8, Ordering},
    RwLock, RwLockReadGuard, RwLockWriteGuard,
};

pub use std::sync::{Arc, Weak};

#[cfg(all(test, not(loom)))]
pub(crate) use std::{
    sync::{atomic::AtomicUsize, Mutex, MutexGuard},
    thread,
};
