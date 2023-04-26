use super::*;

pub trait CrdtList {
    type Item;

    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<&Self::Item>;
    fn insert(&mut self, index: usize, item: Self::Item);
    fn remove(&mut self, index: usize) -> Option<Self::Item>;
    fn push(&mut self, item: Self::Item);
    fn pop(&mut self) -> Option<Self::Item>;
    fn truncate(&mut self, len: usize);
    fn clear(&mut self);

    fn get_store(&self) -> DocStore;
}

pub trait CrdtMap {
    type Key;
    type Value;

    fn len(&self) -> usize;
    fn get(&self, key: &Self::Key) -> Option<&Self::Value>;
    fn insert(&mut self, key: Self::Key, value: Self::Value);
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value>;
    fn clear(&mut self);

    fn get_store(&self) -> DocStore;
}
