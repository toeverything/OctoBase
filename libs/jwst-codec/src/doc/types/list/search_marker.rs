use super::*;
use std::{cmp::max, collections::LinkedList};

const MAX_SEARCH_MARKER: usize = 80;

struct SearchMarker {
    ptr: Box<Item>,
    index: u64,
}

impl SearchMarker {
    fn new(ptr: Box<Item>, index: u64) -> Self {
        SearchMarker { ptr, index }
    }

    fn overwrite_marker(&mut self, ptr: Box<Item>, index: u64) {
        self.ptr = ptr;
        self.index = index;
    }
}

pub struct MarkerList {
    // in yjs, a timestamp field is used to sort markers and the oldest marker is deleted once the limit is reached.
    // this was designed for optimization purposes for v8. In Rust, we can simply use a linked list and trust the compiler to optimize.
    // the linked list can naturally maintain the insertion order, allowing us to know which marker is the oldest without using an extra timestamp field.
    search_marker: LinkedList<SearchMarker>,
    store: DocStore,
}

impl MarkerList {
    fn new(store: DocStore) -> Self {
        MarkerList {
            search_marker: LinkedList::new(),
            store,
        }
    }

    fn mark_position(&mut self, ptr: Box<Item>, index: u64) {
        if self.search_marker.len() >= MAX_SEARCH_MARKER {
            let mut oldest_marker = self.search_marker.pop_front().unwrap();
            oldest_marker.overwrite_marker(ptr, index);
            self.search_marker.push_back(oldest_marker);
        } else {
            let marker = SearchMarker::new(ptr, index);
            self.search_marker.push_back(marker);
        }
    }

    fn update_marker_changes(&mut self, index: u64, len: i64) {
        for marker in self.search_marker.iter_mut() {
            let mut ptr = marker.ptr.clone();
            if len > 0 {
                while let Some(left_item) = ptr.left_id {
                    if let Ok(left_item) = self.store.get_item(left_item) {
                        if let Some(left_item) = left_item.as_ref().as_item() {
                            ptr = left_item;
                            if !ptr.deleted() && ptr.content.countable() {
                                marker.index -= ptr.content.clock_len();
                            }
                        }
                    }
                }
                marker.ptr = ptr;
            }
            if index < marker.index || (len > 0 && index == marker.index) {
                marker.index = max(index, (marker.index as i64 + len) as u64);
            }
        }
    }
}
