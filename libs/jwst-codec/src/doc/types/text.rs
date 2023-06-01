use crate::{
    doc::{item_flags, DocStore, ItemFlags, Parent, StructInfo},
    impl_type, Content, Item, JwstCodecError, JwstCodecResult,
};
use std::sync::Arc;

pub struct ItemPosition {
    pub left: Option<StructInfo>,
    pub right: Option<StructInfo>,
    pub offset: u64,
}

impl_type!(Text);

impl Text {
    pub fn len(&self) -> u64 {
        self.read().content_len
    }

    pub fn insert<T: ToString>(&mut self, char_index: u64, str: T) -> JwstCodecResult {
        let mut inner = self.write();
        if char_index > inner.content_len {
            return Err(JwstCodecError::IndexOutOfBound(char_index));
        }

        if let Some(store) = inner.store.upgrade() {
            let mut store = store.write().unwrap();
            if let Some(mut pos) = self.find_position(&store, inner.start.clone(), char_index) {
                // insert in between an splitable item.
                if pos.offset > 0 {
                    debug_assert!(pos.left.is_some());
                    let (left, right) = store.split_item(pos.left.unwrap().id(), pos.offset)?;
                    pos.left = Some(left);
                    pos.right = Some(right);
                    pos.offset = 0;
                    inner.item_len += 1;
                }

                // insert new item
                let new_item_id = (store.client(), store.get_state(store.client())).into();
                // replace with char len
                let item = StructInfo::Item(Arc::new(Item {
                    id: new_item_id,
                    left_id: pos.left.map(|l| l.id() + l.len() - 1),
                    right_id: pos.right.map(|r| r.id()),
                    content: Arc::new(Content::String(str.to_string())),
                    parent: Some(Parent::Type(self.inner())),
                    parent_sub: None,
                    flags: ItemFlags::from(item_flags::ITEM_COUNTABLE),
                }));

                store.integrate_struct_info(item, 0, Some(inner))?;
                // TODO: deal with text attributes
            }
        }

        Ok(())
    }

    fn find_position(
        &self,
        store: &DocStore,
        start: Option<StructInfo>,
        char_index: u64,
    ) -> Option<ItemPosition> {
        let mut remaining = char_index;
        let mut pos = ItemPosition {
            left: None,
            right: start,
            offset: 0,
        };

        while let Some(cur) = pos.right.take() {
            if remaining == 0 {
                break;
            }

            if let StructInfo::Item(item) = &cur {
                if !item.deleted() {
                    let content_len = item.len();
                    if remaining < content_len {
                        pos.offset = remaining;
                        remaining = 0;
                    } else {
                        remaining -= content_len;
                    }
                }

                pos.right = item.right_id.and_then(|right_id| store.get_item(right_id));
                pos.left = Some(cur);
            } else {
                return None;
            }
        }

        Some(pos)
    }
}

impl ToString for Text {
    fn to_string(&self) -> String {
        let inner = self.read();

        let mut start = inner.start.clone();
        let mut ret = String::with_capacity(inner.content_len as usize);

        while let Some(cur) = start.take() {
            match cur {
                StructInfo::Item(item) => {
                    if !item.deleted() {
                        if let Content::String(str) = item.content.as_ref() {
                            ret.push_str(str);
                        }
                        start = item.right_id.and_then(|right_id| {
                            inner
                                .store
                                .upgrade()
                                .unwrap()
                                .read()
                                .unwrap()
                                .get_item(right_id)
                        });
                    }
                }
                _ => break,
            }
        }

        ret
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::Doc;

    #[test]
    fn test_manipulate_text() {
        let mut doc = Doc::default();
        let mut text = doc.create_text().unwrap();

        text.insert(0, "hello").unwrap();
        text.insert(5, " world").unwrap();
        text.insert(6, "great ").unwrap();

        assert_eq!(text.to_string(), "hello great world");
    }

    #[test]
    fn test_parallel_manipulate_text() {
        let mut doc = Doc::default();
        let mut text = doc.get_or_crate_text("test").unwrap();
        text.insert(0, "This is a string with length 32.").unwrap();
        let mut handles = Vec::new();
        let iteration = 10;

        // parallel editing text
        {
            for i in 0..iteration {
                let mut text = text.clone();
                handles.push(std::thread::spawn(move || {
                    let pos = rand::thread_rng().gen_range(0..text.len());
                    text.insert(pos, format!("hello {i}")).unwrap();
                }));
            }
        }

        // parallel editing doc
        {
            for i in 0..iteration {
                let mut doc = doc.clone();
                handles.push(std::thread::spawn(move || {
                    let mut text = doc.get_or_crate_text("test").unwrap();
                    let pos = rand::thread_rng().gen_range(0..text.len());
                    text.insert(pos, format!("hello doc{i}")).unwrap();
                }));
            }
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(
            text.to_string().len(),
            32 + /* raw length */
            7 * iteration +/* parallel text editing: insert(pos, "hello {i}") */
            10 * iteration /* parallel doc editing: insert(pos, "hello doc{i}") */
        );
    }
}
