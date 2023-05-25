use std::{
    ops::Deref,
    sync::{RwLock, Weak},
};

use crate::{
    doc::{item_flags, DocStore, ItemFlags, Parent, StructInfo, StructRef},
    Content, Item, JwstCodecResult,
};

pub type DocStoreRef = Weak<RwLock<DocStore>>;

pub struct ItemPosition {
    pub left: Option<StructRef>,
    pub right: Option<StructRef>,
    pub offset: u64,
}

struct YText {
    start: Option<StructRef>,
    item: Option<StructRef>,
    len: u64,
    store: DocStoreRef,
}

impl YText {
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn insert(&mut self, char_index: u64, str: String) -> JwstCodecResult {
        if char_index > self.len {
            panic!("index out of bounds");
        }
        debug_assert!(self.start.is_some());

        if let Some(store) = self.store.upgrade() {
            let mut store = store.write().unwrap();
            if let Some(mut pos) =
                self.find_position(&store, self.start.as_ref().unwrap().clone(), char_index)
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
                let item = StructInfo::Item {
                    id: new_item_id,
                    item: Box::new(Item {
                        id: new_item_id,
                        left_id: pos.left.map(|l| l.id()),
                        right_id: pos.right.map(|r| r.id()),
                        content: Content::String(str),
                        parent: Some(if let Some(parent_item) = self.item.as_ref() {
                            Parent::Id(parent_item.id())
                        } else {
                            // FIXME: how could we get root parent name here?
                            Parent::String("".into())
                        }),
                        parent_sub: None,
                        flags: ItemFlags::from(item_flags::ITEM_COUNTABLE),
                    }),
                };

                store.integrate_struct_info(item, 0)?;
                // TODO: deal with text attributes
            }
        }

        Ok(())
    }

    fn find_position(
        &self,
        store: &DocStore,
        start: StructRef,
        char_index: u64,
    ) -> Option<ItemPosition> {
        let mut remaining = char_index;
        let mut pos = ItemPosition {
            left: None,
            right: Some(start),
            offset: 0,
        };

        while let Some(cur) = pos.right.take() {
            if remaining == 0 {
                break;
            }

            if let StructInfo::Item { item, .. } = cur.borrow().deref() {
                if !item.deleted() {
                    let content_len = item.content.clock_len();
                    if remaining < content_len {
                        pos.offset = remaining;
                        remaining = 0;
                    } else {
                        remaining -= content_len;
                    }
                }

                pos.right = item.right_id.and_then(|right_id| {
                    if let Ok(r) = store.get_item(right_id) {
                        Some(r)
                    } else {
                        None
                    }
                });
                pos.left = Some(cur.clone());
            } else {
                return None;
            }
        }

        Some(pos)
    }
}
