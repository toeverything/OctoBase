// #[cfg(loom)]
// pub(crate) use loom::sync::{
//     atomic::{AtomicU8, AtomicUsize, Ordering},
//     Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak,
// };

// #[cfg(not(loom))]
pub(crate) use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak,
};

#[cfg(test)]
pub(crate) use std::sync::{atomic::AtomicUsize, Mutex, MutexGuard};
