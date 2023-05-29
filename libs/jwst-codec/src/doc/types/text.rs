use std::sync::Arc;

use crate::{
    doc::{item_flags, DocStore, ItemFlags, Parent, StructInfo},
    impl_type, Content, Item, JwstCodecResult,
};

pub struct ItemPosition {
    pub left: Option<StructInfo>,
    pub right: Option<StructInfo>,
    pub offset: u64,
}

impl_type!(Text);

impl Text {
    pub fn len(&self) -> u64 {
        self.0.read().unwrap().content_len
    }

    pub fn insert(&mut self, char_index: u64, str: String) -> JwstCodecResult {
        let type_store = self.0.write().unwrap();
        if char_index > type_store.content_len {
            panic!("index out of bounds");
        }

        if let Some(store) = type_store.store.upgrade() {
            let mut store = store.write().unwrap();
            if let Some(mut pos) = self.find_position(&store, type_store.start.clone(), char_index)
            {
                // insert in between an splitable item.
                if pos.offset > 0 {
                    debug_assert!(pos.left.is_some());
                    let (left, right) = store.split_item(pos.left.unwrap().id(), pos.offset)?;
                    pos.left = Some(left);
                    pos.right = Some(right);
                    pos.offset = 0;
                }

                // insert new item
                let new_item_id = (store.client(), store.get_state(store.client())).into();
                let item = StructInfo::Item(Arc::new(Item {
                    id: new_item_id,
                    left_id: pos.left.map(|l| l.id()),
                    right_id: pos.right.map(|r| r.id()),
                    content: Arc::new(Content::String(str)),
                    parent: Some(if let Some(parent_item) = type_store.item.as_ref() {
                        Parent::Id(parent_item.id())
                    } else {
                        // FIXME: how could we get root parent name here?
                        Parent::String("".into())
                    }),
                    parent_sub: None,
                    flags: ItemFlags::from(item_flags::ITEM_COUNTABLE),
                }));

                store.integrate_struct_info(item, 0)?;
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
