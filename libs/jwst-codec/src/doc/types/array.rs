use super::*;

pub struct YArray {
    core: ListCore,
}

impl YArray {
    pub fn new(store: DocStore, root: TypeStoreRef) -> YArray {
        Self {
            core: ListCore::new(store, root),
        }
    }

    pub fn get(&self, index: u64) -> JwstCodecResult<Option<Content>> {
        self.core.get(index)
    }
}
