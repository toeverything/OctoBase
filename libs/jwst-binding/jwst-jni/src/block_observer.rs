pub trait BlockObserver {
    fn on_change(&self, block_ids: Vec<String>);
}

pub(crate) struct BlockObserverWrapper {
    cb: Box<dyn BlockObserver>,
}

impl BlockObserverWrapper {
    pub fn new(cb: Box<dyn BlockObserver>) -> Self {
        Self { cb }
    }
}

impl BlockObserver for BlockObserverWrapper {
    fn on_change(&self, block_ids: Vec<String>) {
        self.cb.on_change(block_ids)
    }
}

unsafe impl Send for BlockObserverWrapper {}
unsafe impl Sync for BlockObserverWrapper {}
