use super::*;

pub struct ListCore {
    store: DocStore,
    root: TypeStoreRef,
    marker_list: MarkerList,
}

impl ListCore {
    pub(crate) fn new(store: DocStore, root: TypeStoreRef) -> ListCore {
        Self {
            store: store.clone(),
            root,
            marker_list: MarkerList::new(store),
        }
    }

    pub(crate) fn get(&self, index: u64) -> JwstCodecResult<Option<Content>> {
        let root = self.root.borrow();
        if let Some(start) = root.start.as_ref().and_then(|s| s.as_item()) {
            let mut index = index;

            let mut item_ptr = match self.marker_list.find_marker(index, start.clone())? {
                Some(marker) => {
                    index -= marker.index;
                    marker.ptr.clone()
                }
                None => start.clone(),
            };

            loop {
                if item_ptr.indexable() {
                    if index < item_ptr.len() {
                        return item_ptr.content.at(index);
                    }
                    index -= item_ptr.len();
                }
                if let Some(right_id) = item_ptr.right_id {
                    if let Some(right_item) = self.store.get_item(right_id)?.as_item() {
                        item_ptr = right_item;
                        continue;
                    }
                }
                break;
            }
        }

        Ok(None)
    }

    pub fn iter(&self) -> ListIterator {
        ListIterator {
            store: self.store.clone(),
            next: self.root.borrow().start.clone(),
            content: None,
            content_idx: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Transact};

    #[test]
    fn test_core_iter() {
        let buffer = {
            let doc = yrs::Doc::new();
            let array = doc.get_or_insert_array("abc");

            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, " ").unwrap();
            array.insert(&mut trx, 0, "Hello").unwrap();
            array.insert(&mut trx, 2, "World").unwrap();
            trx.encode_update_v1().unwrap()
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();
        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();
        let array = doc.get_array("abc").unwrap();

        let items = array.iter().flatten().collect::<Vec<_>>();
        assert_eq!(
            items,
            vec![
                Content::Any(vec![Any::String("Hello".into())]),
                Content::Any(vec![Any::String(" ".into())]),
                Content::Any(vec![Any::String("World".into())]),
            ]
        );
    }
}
