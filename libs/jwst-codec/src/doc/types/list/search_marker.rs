use super::*;
use std::{
    cell::RefCell,
    cmp::max,
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

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

/// in yjs, a timestamp field is used to sort markers and the oldest marker is deleted once the limit is reached.
/// this was designed for optimization purposes for v8. In Rust, we can simply use a [VecDeque] and trust the compiler to optimize.
/// the [VecDeque] can naturally maintain the insertion order, allowing us to know which marker is the oldest without using an extra timestamp field.
///
/// NOTE:
/// A [MarkerList] is always belonging to a [YType],
/// which means whenever [MakerList] is used, we actually have a [YType] instance behind [RwLock] guard already,
/// so it's safe to make the list internal mutable.
#[derive(Debug)]
pub struct MarkerList(RefCell<VecDeque<SearchMarker>>);

impl Deref for MarkerList {
    type Target = RefCell<VecDeque<SearchMarker>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MarkerList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl MarkerList {
    pub(crate) fn new() -> Self {
        MarkerList(RefCell::new(VecDeque::new()))
    }

    // mark pos and push to the end of the linked list
    fn mark_position(
        list: &mut VecDeque<SearchMarker>,
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
    pub(super) fn update_marker_changes(&self, index: u64, len: i64) {
        let mut list = self.borrow_mut();

        for marker in list.iter_mut() {
            let mut ptr = marker.ptr.clone();
            // current marker item has been deleted
            // we should move the pos to prev undeleted item
            if len > 0 {
                while !ptr.indexable() {
                    if let Some(StructInfo::Item(left)) = &ptr.left {
                        ptr = left.clone();
                        if ptr.indexable() {
                            marker.index -= ptr.len();
                        }
                    } else {
                        // remove marker
                        marker.index = 0;
                        break;
                    }
                }
            }
            if index < marker.index || (len > 0 && index == marker.index) {
                marker.index = max(index as i64, marker.index as i64 + len) as u64;
            }
            marker.ptr = ptr;
        }

        list.retain(|marker| marker.index > 0);
    }

    // find and return the marker that is closest to the index
    pub(super) fn find_marker(&self, parent: &YType, index: u64) -> Option<SearchMarker> {
        if parent.start().is_none() || index == 0 {
            return None;
        }

        let mut list = self.borrow_mut();

        let marker = list
            .iter_mut()
            .min_by_key(|m| (index as i64 - m.index as i64).abs());

        let mut marker_index = marker.as_ref().map(|m| m.index).unwrap_or(0);

        let mut item_ptr = marker
            .as_ref()
            .map_or_else(|| parent.start().unwrap(), |m| m.ptr.clone());

        // TODO: this logic here is a bit messy
        // i think it can be implemented with more streamlined code, and then optimized
        {
            // iterate to the right if possible
            while let Some(StructInfo::Item(right_item)) = &item_ptr.right {
                if marker_index >= index {
                    break;
                }

                if item_ptr.indexable() {
                    if index < marker_index + item_ptr.len() {
                        break;
                    }
                    marker_index += item_ptr.len();
                }
                item_ptr = right_item.clone();
            }

            // iterate to the left if necessary (might be that marker_index > index)
            while let Some(StructInfo::Item(left_item)) = &item_ptr.left {
                if marker_index <= index {
                    break;
                }

                item_ptr = left_item.clone();
                if item_ptr.indexable() {
                    marker_index -= item_ptr.len();
                }
            }

            // we want to make sure that item_ptr can't be merged with left, because that would screw up everything
            // in that case just return what we have (it is most likely the best marker anyway)
            // iterate to left until item_ptr can't be merged with left
            while let Some(StructInfo::Item(left_item)) = &item_ptr.left {
                if left_item.id.client == item_ptr.id.client
                    && left_item.id.clock + left_item.len() == item_ptr.id.clock
                {
                    item_ptr = left_item.clone();
                    if item_ptr.indexable() {
                        marker_index -= item_ptr.len();
                    }
                    continue;
                }
                break;
            }
        }

        match marker {
            Some(marker)
                if (marker.index as f64 - marker_index as f64).abs()
                    < parent.len as f64 / MAX_SEARCH_MARKER as f64 =>
            {
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
        self.borrow().back().cloned()
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
            let doc = yrs::Doc::with_client_id(1);
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
        let array = doc.get_or_create_array("abc").unwrap();

        let marker_list = MarkerList::new();

        let marker = marker_list.find_marker(&array.read(), 8).unwrap();

        assert_eq!(marker.index, 2);
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
    fn test_search_marker_flaky() {
        let doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "0").unwrap();
        text.insert(1, "1").unwrap();
        text.insert(0, "0").unwrap();
    }

    fn search_with_seed(seed: u64) {
        let rand = ChaCha20Rng::seed_from_u64(seed);
        let iteration = 20;

        let doc = Doc::with_client(1);
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "This is a string with length 32.").unwrap();
        let mut len = text.len();

        for i in 0..iteration {
            let mut rand: ChaCha20Rng = rand.clone();
            let pos = rand.gen_range(0..text.len());
            let str = format!("hello {i}");
            len += str.len() as u64;
            text.insert(pos, str).unwrap();
        }

        assert_eq!(text.len(), len);
        assert_eq!(text.to_string().len() as u64, len);
    }

    #[test]
    fn test_marker_list_with_seed() {
        search_with_seed(785590655803394607);
        search_with_seed(12958877733367615);
        search_with_seed(71776330571528794);
        search_with_seed(2207805473582911);
    }
}
