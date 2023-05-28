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
}
