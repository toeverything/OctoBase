mod core;
mod iterator;
mod packed_content;
mod search_marker;

use std::sync::RwLockWriteGuard;

pub use self::core::{Array, ListCore, XMLFragment};
pub use iterator::ListIterator;

use super::*;
use packed_content::PackedContent;
pub use search_marker::MarkerList;

pub(crate) struct ItemPosition {
    pub parent: YTypeRef,
    pub left: Option<ItemRef>,
    pub right: Option<ItemRef>,
    pub offset: u64,
}

struct Insert<'a> {
    store: &'a mut DocStore,
    lock: RwLockWriteGuard<'a, YType>,
    pos: ItemPosition,
    content: Content,
}

impl Insert<'_> {
    fn apply(self) -> JwstCodecResult {
        let Insert {
            store,
            mut lock,
            mut pos,
            content,
        } = self;
        // insert in between an splitable item.
        if pos.offset > 0 {
            debug_assert!(pos.left.is_some());
            let (_, right) = store.split_item(pos.left.as_ref().unwrap().id, pos.offset)?;
            pos.right = right.as_item();
            lock.item_len += 1;
        }

        // insert new item
        let new_item_id = (store.client(), store.get_state(store.client())).into();
        // replace with char len
        let item = StructInfo::Item(Arc::new(Item {
            id: new_item_id,
            left_id: pos.left.map(|l| l.id),
            right_id: pos.right.map(|r| r.id),
            content: Arc::new(content),
            parent: Some(Parent::Type(pos.parent)),
            parent_sub: None,
            flags: ItemFlags::from(item_flags::ITEM_COUNTABLE),
        }));

        store.integrate_struct_info(item, 0, Some(lock))
    }
}

pub(crate) trait ListType: AsInner<Inner = YTypeRef> {
    fn len(&self) -> u64;
    fn insert_content_at(&mut self, index: u64, content: Content) -> JwstCodecResult {
        if index > self.len() {
            return Err(JwstCodecError::IndexOutOfBound(index));
        }

        if let Some(pos) = self.find_pos(index) {
            let inner = self.as_inner().write().unwrap();
            if let Some(store) = inner.store.upgrade() {
                let mut store = store.write().unwrap();
                let insert = Insert {
                    store: &mut store,
                    lock: inner,
                    pos,
                    content,
                };

                return insert.apply();
            }
        }

        Ok(())
    }

    fn find_pos(&self, index: u64) -> Option<ItemPosition> {
        let this = self.as_inner();
        let inner = this.read().unwrap();
        if let Some(store) = inner.store.upgrade() {
            let store = store.read().unwrap();
            let mut remaining = index;

            // TODO: use marker
            let mut pos = ItemPosition {
                parent: this.clone(),
                left: None,
                right: inner.start.as_ref().and_then(|s| s.as_item()),
                offset: 0,
            };

            while let Some(item) = pos.right.take() {
                if remaining == 0 {
                    break;
                }

                if !item.deleted() {
                    let content_len = item.len();
                    if remaining < content_len {
                        pos.offset = remaining;
                        remaining = 0;
                    } else {
                        remaining -= content_len;
                    }
                }

                pos.right = item
                    .right_id
                    .and_then(|right_id| store.get_item(right_id))
                    .and_then(|r| r.as_item());
                pos.left = Some(item);
            }

            Some(pos)
        } else {
            None
        }
    }
}
