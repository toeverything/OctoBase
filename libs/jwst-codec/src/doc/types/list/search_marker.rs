use super::*;
use std::{cmp::max, collections::LinkedList, sync::Mutex};

const MAX_SEARCH_MARKER: usize = 80;

#[derive(Clone, Debug)]
pub struct SearchMarker {
    pub(super) ptr: Arc<Item>,
    pub(super) index: u64,
}

impl SearchMarker {
    fn new(ptr: Arc<Item>, index: u64) -> Self {
        SearchMarker { ptr, index }
    }

    fn overwrite_marker(&mut self, ptr: Arc<Item>, index: u64) {
        self.ptr = ptr;
        self.index = index;
    }
}

#[derive(Debug)]
pub struct MarkerList {
    // in yjs, a timestamp field is used to sort markers and the oldest marker is deleted once the limit is reached.
    // this was designed for optimization purposes for v8. In Rust, we can simply use a linked list and trust the compiler to optimize.
    // the linked list can naturally maintain the insertion order, allowing us to know which marker is the oldest without using an extra timestamp field.
    search_marker: Arc<Mutex<LinkedList<SearchMarker>>>,
}

impl MarkerList {
    pub(crate) fn new() -> Self {
        MarkerList {
            search_marker: Arc::new(Mutex::new(LinkedList::new())),
        }
    }

    // mark pos and push to the end of the linked list
    fn mark_position(
        list: &mut LinkedList<SearchMarker>,
        ptr: Arc<Item>,
        index: u64,
    ) -> Option<SearchMarker> {
        if list.len() >= MAX_SEARCH_MARKER {
            let mut oldest_marker = list.pop_front().unwrap();
            oldest_marker.overwrite_marker(ptr, index);
            list.push_back(oldest_marker);
        } else {
            let marker = SearchMarker::new(ptr, index);
            list.push_back(marker);
        }
        list.back().cloned()
    }

    // update mark position if the index is within the range of the marker
    pub(super) fn update_marker_changes(&self, store: &DocStore, index: u64, len: i64) {
        let mut list = self.search_marker.lock().unwrap();
        for marker in list.iter_mut() {
            let mut ptr = marker.ptr.clone();
            if len > 0 {
                while let Some(left_item) = ptr.left(store) {
                    ptr = left_item;
                    if ptr.indexable() {
                        marker.index -= ptr.len();
                    }
                }
                marker.ptr = ptr;
            }
            if index < marker.index || (len > 0 && index == marker.index) {
                marker.index = max(index, (marker.index as i64 + len) as u64);
            }
        }
    }

    // find and return the marker that is closest to the index
    pub(super) fn find_marker(
        &self,
        store: &DocStore,
        index: u64,
        parent_start: Arc<Item>,
    ) -> Option<SearchMarker> {
        let items = store.items.read().unwrap();
        if items.is_empty() || index == 0 {
            return None;
        }

        let mut list = self.search_marker.lock().unwrap();

        let marker = list
            .iter_mut()
            .min_by_key(|m| (index as i64 - m.index as i64).abs());
        let mut marker_index = marker.as_ref().map(|m| m.index).unwrap_or(0);
        let mut item_ptr = marker.as_ref().map_or(parent_start, |m| m.ptr.clone());

        // TODO: this logic here is a bit messy
        // i think it can be implemented with more streamlined code, and then optimized
        {
            // iterate to the right if possible
            #[allow(clippy::never_loop)]
            while let Some(right_item) = item_ptr.right(store) {
                if marker_index < index {
                    if item_ptr.indexable() {
                        if index < marker_index + item_ptr.len() {
                            break;
                        }
                        marker_index += item_ptr.len();
                    }
                    item_ptr = right_item;
                    // TODO: waiting faster left/right refactoring
                    // continue;
                }
                break;
            }

            // iterate to the left if necessary (might be that marker_index > index)
            #[allow(clippy::never_loop)]
            while let Some(left_item) = item_ptr.left(store) {
                if marker_index > index {
                    item_ptr = left_item;
                    if item_ptr.indexable() {
                        let item_len = item_ptr.len();
                        if marker_index > item_len {
                            marker_index -= item_ptr.len();
                        }
                    }
                    // TODO: waiting faster left/right refactoring
                    // continue;
                }
                break;
            }

            // we want to make sure that item_ptr can't be merged with left, because that would screw up everything
            // in that case just return what we have (it is most likely the best marker anyway)
            // iterate to left until item_ptr can't be merged with left
            while let Some(left_item) = item_ptr.left(store) {
                if left_item.id.client == item_ptr.id.client
                    && left_item.id.clock + item_ptr.len() == item_ptr.id.clock
                {
                    item_ptr = left_item;
                    if item_ptr.indexable() {
                        marker_index -= item_ptr.len();
                    }
                    continue;
                }
                break;
            }
        }

        match marker {
            // FIXME: get_parent requires a lock and would result in deadlock
            // the original logic will filter out the update of the too short clock
            // due to the deadlock, we will ignore the problem first
            // if there is a performance problem, deal with the problem
            // Some(marker)
            //     if parent_start
            //         .get_parent(&store)
            //         .ok()
            //         .flatten()
            //         .map(|ptr| {
            //             (marker.index as i64 - marker_index as i64).abs()
            //                 < ptr.len() as i64 / MAX_SEARCH_MARKER as i64
            //         })
            //         .unwrap_or(false) =>
            Some(marker) => {
                // adjust existing marker
                marker.overwrite_marker(item_ptr, marker_index);
                Some(marker.clone())
            }
            _ => {
                // create new marker
                Self::mark_position(&mut list, item_ptr, marker_index)
            }
        }
    }

    #[allow(dead_code)]
    pub(super) fn get_last_marker(&self) -> Option<SearchMarker> {
        self.search_marker.lock().unwrap().back().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use yrs::{Array, Transact};

    #[test]
    fn test_marker_list() {
        let (client_id, buffer) = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, " ").unwrap();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 2, "World").unwrap();
            (doc.client_id(), trx.encode_update_v1().unwrap())
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();

        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();

        let marker_list = MarkerList::new();

        let marker = marker_list
            .find_marker(
                &doc.store.read().unwrap(),
                8,
                doc.store
                    .read()
                    .unwrap()
                    .get_item(Id::new(client_id, 0))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(marker.index, 1);
        assert_eq!(
            marker.ptr,
            doc.store
                .read()
                .unwrap()
                .get_item(Id::new(client_id, 2))
                .unwrap()
        );
    }

    #[test]
    #[ignore = "temp for local test"]
    fn test_search_marker_flaky() {
        let doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "0").unwrap();
        text.insert(1, "1").unwrap();
        text.insert(0, "0").unwrap();
    }

    fn search_with_seed(seed: u64) {
        let rand = ChaCha20Rng::seed_from_u64(seed);
        let iteration = 10;

        let doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "This is a string with length 32.").unwrap();

        for i in 0..iteration {
            let mut text = text.clone();
            let mut rand = rand.clone();
            let pos = rand.gen_range(0..text.len());
            text.insert(pos, format!("hello {i}")).unwrap();
        }

        for i in 0..iteration {
            let doc = doc.clone();
            let mut rand = rand.clone();
            let mut text = doc.get_or_crate_text("test").unwrap();
            let pos = rand.gen_range(0..text.len());
            text.insert(pos, format!("hello doc{i}")).unwrap();
        }

        assert_eq!(
            text.to_string().len(),
            32 /* raw length */
        + 7 * iteration /* parallel text editing: insert(pos, "hello {i}") */
        + 10 * iteration /* parallel doc editing: insert(pos, "hello doc{i}") */
        );
    }

    #[test]
    fn test_marker_list_with_seed() {
        // search_with_seed(785590655803394607);
        search_with_seed(12958877733367615);
        search_with_seed(71776330571528794);
        search_with_seed(2207805473582911);
    }
}
