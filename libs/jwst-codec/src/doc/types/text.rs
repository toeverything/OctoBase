use crate::Item;

use super::{TypeStore, TypeStoreKind, TypeStoreRef};

pub struct YText {
    inner: TypeStoreRef,
}

impl YText {
    pub fn new() -> Self {
        Self {
            inner: TypeStore::new(TypeStoreKind::Text).into(),
        }
    }
}

impl From<String> for YText {
    fn from(value: String) -> Self {
        let mut text = Self::new();
        text.insert(0, value);

        text
    }
}

impl YText {
    pub fn len(&self) -> usize {
        self.inner.borrow().len
    }

    pub fn insert(&mut self, index: usize, text: String) {
        let pos = self.find_position(index);
    }

    fn find_position(&self, index: usize) -> Option<Item> {
        todo!()
    }
}
