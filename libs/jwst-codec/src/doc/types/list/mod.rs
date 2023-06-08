mod iterator;
mod search_marker;

use std::sync::RwLockWriteGuard;

pub use iterator::ListIterator;

use super::*;
pub use search_marker::MarkerList;

pub(crate) struct ItemPosition {
    pub parent: YTypeRef,
    pub left: Option<ItemRef>,
    pub right: Option<ItemRef>,
    pub index: u64,
    pub offset: u64,
}

pub(crate) trait ListType: AsInner<Inner = YTypeRef> {
    #[inline(always)]
    fn content_len(&self) -> u64 {
        self.as_inner().read().unwrap().len
    }

    fn iter_item(&self) -> ListIterator<'_> {
        let inner = self.as_inner().read().unwrap();
        ListIterator {
            store: inner.store().unwrap(),
            next: inner.start.clone(),
        }
    }

    fn find_pos(&self, inner: &YType, store: &DocStore, index: u64) -> ItemPosition {
        let mut remaining = index;
        let mut pos = ItemPosition {
            parent: self.as_inner().clone(),
            left: None,
            right: inner.start(),
            index: 0,
            offset: 0,
        };

        if pos.right.is_none() {
            return pos;
        }

        if let Some(markers) = &inner.markers {
            if let Some(marker) = markers.find_marker(store, index, pos.right.clone().unwrap()) {
                remaining -= marker.index;
                pos.index = marker.index;
                pos.right = Some(marker.ptr);
            }
        };

        while remaining > 0 {
            if let Some(item) = pos.right.take() {
                if !item.deleted() {
                    let content_len = item.len();
                    if remaining < content_len {
                        pos.offset = remaining;
                        pos.index += remaining;
                        remaining = 0;
                    } else {
                        pos.index += content_len;
                        remaining -= content_len;
                    }
                }

                pos.right = item.right(store);
                pos.left = Some(item);
            } else {
                break;
            }
        }

        pos
    }

    fn insert_at(&mut self, index: u64, contents: Vec<Content>) -> JwstCodecResult {
        if index > self.content_len() {
            return Err(JwstCodecError::IndexOutOfBound(index));
        }

        let mut inner = self.as_inner().write().unwrap();
        if let Some(mut store) = inner.store_mut() {
            let pos = self.find_pos(&inner, &store, index);
            Self::insert_after(inner, &mut store, pos, contents)?;
        }

        Ok(())
    }

    fn insert_after(
        mut lock: RwLockWriteGuard<YType>,
        store: &mut DocStore,
        mut pos: ItemPosition,
        contents: Vec<Content>,
    ) -> JwstCodecResult {
        // insert in between an splitable item.
        if pos.offset > 0 {
            debug_assert!(pos.left.is_some());
            let (_, right) = store.split_node(pos.left.as_ref().unwrap().id, pos.offset)?;
            pos.right = right.as_item();
        }

        let mut len = 0;

        for content in contents {
            let new_item_id = (store.client(), store.get_state(store.client())).into();
            let item = Arc::new(
                ItemBuilder::new()
                    .id(new_item_id)
                    .left_id(pos.left.map(|l| l.id))
                    .right_id(pos.right.clone().map(|r| r.id))
                    .content(content)
                    .parent(Some(Parent::Type(pos.parent.clone())))
                    .build(),
            );
            store.integrate_struct_info(StructInfo::Item(item.clone()), 0, Some(&mut lock))?;
            len += item.len();
            pos.left = Some(item);
        }

        if let Some(markers) = &lock.markers {
            markers.update_marker_changes(store, pos.index, len as i64);
        }

        Ok(())
    }

    fn get_item_at(&self, index: u64) -> Option<(ItemRef, u64)> {
        if index >= self.content_len() {
            return None;
        }

        let inner = self.as_inner().read().unwrap();
        if let Some(store) = inner.store() {
            let pos = self.find_pos(&inner, &store, index);

            if pos.offset == 0 {
                return pos.right.map(|r| (r, 0));
            } else {
                return pos.left.map(|l| (l, pos.offset));
            }
        }

        None
    }

    fn remove_at(&mut self, idx: u64, len: u64) -> JwstCodecResult {
        if len == 0 {
            return Ok(());
        }

        if idx >= self.content_len() {
            return Err(JwstCodecError::IndexOutOfBound(idx));
        }

        let mut inner = self.as_inner().write().unwrap();
        if let Some(mut store) = inner.store_mut() {
            let pos = self.find_pos(&inner, &store, idx);
            Self::remove_after(inner, &mut store, pos, len)?;
        }

        Ok(())
    }

    fn remove_after(
        mut lock: RwLockWriteGuard<YType>,
        store: &mut DocStore,
        mut pos: ItemPosition,
        len: u64,
    ) -> JwstCodecResult {
        let mut remaining = len as i64;

        // delete in between an splitable item.
        if pos.offset > 0 {
            debug_assert!(pos.left.is_some());
            let (_, right) = store.split_node(pos.left.as_ref().unwrap().id, pos.offset)?;
            pos.right = right.as_item();
        }

        while remaining > 0 {
            if let Some(item) = pos.right.take() {
                if !item.deleted() {
                    let content_len = item.len() as i64;
                    if remaining < content_len {
                        store.split_node(item.id, remaining as u64)?;
                    }
                    remaining -= content_len;
                    store.delete_item(&item, Some(&mut lock));
                }

                pos.right = item
                    .right_id
                    .and_then(|right_id| store.get_node(right_id))
                    .and_then(|right| right.as_item());
                pos.left = Some(item);
            } else {
                break;
            }
        }

        if let Some(markers) = &lock.markers {
            markers.update_marker_changes(store, pos.index, -(len as i64) + remaining);
        }

        Ok(())
    }
}
