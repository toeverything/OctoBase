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

#[macro_export(local_inner_macros)]
macro_rules! loom_model {
    ($test:block) => {
        #[cfg(loom)]
        loom::model(move || $test);

        #[cfg(not(loom))]
        $test
    };
}
