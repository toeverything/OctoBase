use super::*;

impl_type!(Array);
impl_type!(XMLFragment);

pub struct ListCore {
    client_id: Client,
    store: StoreRef,
    root: Option<StructInfo>,
    root_name: Option<String>,
    marker_list: MarkerList,
}

impl ListCore {
    pub(crate) fn new(client_id: Client, list_store: YTypeRef) -> ListCore {
        let list_store = list_store.read().unwrap();
        let store = list_store.store.upgrade().unwrap();
        let root = list_store.start.clone();
        let root_name = list_store.root_name.clone();

        Self {
            client_id,
            store: store.clone(),
            root,
            root_name,
            marker_list: MarkerList::new(store),
        }
    }

    pub(crate) fn get(&self, index: u64) -> JwstCodecResult<Option<Content>> {
        if let Some(start) = self.root.as_ref().and_then(|s| s.as_item()) {
            let mut index = index;

            let mut item_ptr = match self.marker_list.find_marker(index, start.clone())? {
                Some(marker) => {
                    index -= marker.index;
                    marker.ptr
                }
                None => start,
            };

            let store = self.store.read().unwrap();
            loop {
                if item_ptr.indexable() {
                    if index < item_ptr.len() {
                        return item_ptr.content.at(index);
                    }
                    index -= item_ptr.len();
                }
                if let Some(right_id) = item_ptr.right_id {
                    if let Some(right_item) = store.get_item(right_id).and_then(|i| i.as_item()) {
                        item_ptr = right_item;
                        continue;
                    }
                }
                break;
            }
        }

        Ok(None)
    }

    pub(crate) fn iter<'a>(&'a self) -> ListIterator<'a> {
        ListIterator {
            store: self.store.read().unwrap(),
            next: self.root.clone(),
            content: None,
            content_idx: 0,
        }
    }

    pub(crate) fn insert_after(
        &self,
        ref_item: Option<Arc<Item>>,
        content: Vec<Any>,
    ) -> JwstCodecResult<()> {
        let store = self.store.read().unwrap();
        let mut content_pack = PackedContent::new(
            self.client_id,
            self.root.clone(),
            self.root_name.clone(),
            &store,
        );
        content_pack.update_ref_item(ref_item);

        for c in content {
            match c {
                #[rustfmt::skip]
                Any::Undefined | Any::Null | Any::Integer(_) | Any::Float32(_) | Any::Float64(_) | Any::BigInt64(_) |
                Any::False | Any::True | Any::String(_) | Any::Object(_) | Any::Array(_) => {
                    content_pack.push(c);
                }
                Any::Binary(binary) => {
                    content_pack.pack()?;
                    content_pack.update_ref_item(content_pack.ref_item.clone());
                    let new_id = Id::new(self.client_id, store.get_state(self.client_id));
                    let new_struct = content_pack.build_item(new_id, |b| {
                        b.content(Content::Binary(binary.clone())).build()
                    });

                    store.add_item(new_struct.clone())?;
                    content_pack.update_ref_item(new_struct.as_item());
                }
            }
        }

        content_pack.pack()?;
        Ok(())
    }

    pub(crate) fn insert(&mut self, index: u64, content: Vec<Any>) -> JwstCodecResult {
        if index == 0 {
            self.marker_list
                .update_marker_changes(index, content.len() as i64);

            return self.insert_after(None, content);
        }

        let mut curr_idx = index;
        let Some(mut item_ptr) = self.root.as_ref().and_then(|s|s.as_item()) else {
            return Err(JwstCodecError::InvalidParent)
        };

        let mut store = self.store.write().unwrap();

        loop {
            if item_ptr.indexable() {
                if curr_idx <= item_ptr.len() {
                    if curr_idx < item_ptr.len() {
                        store.split_at_and_get_right({
                            let id = item_ptr.id;
                            // split item
                            Id::new(id.client, id.clock + curr_idx)
                        })?;
                    }
                    break;
                }
                curr_idx -= item_ptr.len();
            }
            if let Some(right_id) = item_ptr.right_id {
                if let Some(right_item) = store.get_item(right_id).and_then(|i| i.as_item()) {
                    item_ptr = right_item;
                    continue;
                }
            }
            break;
        }

        self.insert_after(Some(item_ptr), content)
    }

    pub(crate) fn push(&mut self, content: Vec<Any>) -> JwstCodecResult {
        let ref_item = &mut {
            self.marker_list
                .get_last_marker()
                .map(|m| m.ptr.clone())
                .or(self.root.as_ref().and_then(|s| s.as_item()))
        };

        let store = self.store.read().unwrap();

        if let Some(right) = ref_item.as_mut() {
            let id = right.right_id;
            while let Some(right_id) = id {
                // TODO: items that have not been repair() may not have the right
                match store.get_item(right_id).and_then(|i| i.as_item()) {
                    Some(item) => ref_item.replace(item),
                    None => break,
                };
            }
        };
        println!("ref_item: {:#?}", ref_item);

        self.insert_after(ref_item.clone(), content)
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
        let array = doc.get_or_crate_array("abc").unwrap();

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

    #[test]
    fn test_core_list() {
        let buffer = {
            let doc = Doc::default();
            let mut array = doc.get_or_crate_array("abc").unwrap();

            array.insert(0, " ").unwrap();
            array.insert(0, "Hello").unwrap();
            array.insert(2, "World").unwrap();
            // array.push("!").unwrap();

            doc.encode_update_v1().unwrap()
        };

        let mut decoder = RawDecoder::new(buffer);
        let update = Update::read(&mut decoder).unwrap();

        let mut doc = Doc::default();
        doc.apply_update(update).unwrap();
        let array = doc.get_or_crate_array("abc").unwrap();

        assert_eq!(
            array.iter().flatten().collect::<Vec<_>>(),
            vec![
                Content::Any(vec![Any::String("Hello".into())]),
                Content::Any(vec![Any::String(" ".into())]),
                Content::Any(vec![Any::String("World".into())]),
                // Content::Any(vec![Any::String("!".into())]),
            ]
        );
        assert_eq!(
            array.slice(1, 3).unwrap(),
            vec![
                Content::Any(vec![Any::String(" ".into())]),
                Content::Any(vec![Any::String("World".into())])
            ]
        );
    }
}
