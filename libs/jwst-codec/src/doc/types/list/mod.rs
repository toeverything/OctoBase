mod iterator;
mod search_marker;

pub use iterator::ListIterator;
pub use search_marker::MarkerList;

use super::*;
use crate::sync::RwLockWriteGuard;

pub(crate) struct ItemPosition {
    pub parent: YTypeWeakRef,
    pub left: Option<Weak<Item>>,
    pub right: Option<Weak<Item>>,
    pub index: u64,
    pub offset: u64,
}

impl ItemPosition {
    pub fn forward(&mut self) {
        if let Some(right) = self.right.take().and_then(|a| a.upgrade()) {
            if !right.deleted() {
                self.index += right.len();
            }

            self.left = Some(Arc::downgrade(&right));
            self.right = right.right.as_ref().and_then(|right| right.as_weak_item());
        } else {
            // FAIL
        }
    }

    /// we found a position cursor point in between a splitable item,
    /// we need to split the item by the offset.
    ///
    /// before:
    /// ---------------------------------
    ///    ^left                ^right
    ///            ^offset
    /// after:
    /// ---------------------------------
    ///    ^left   ^right
    ///
    pub fn normalize(&mut self, store: &mut DocStore) -> JwstCodecResult {
        if self.offset > 0 {
            debug_assert!(self.left.is_some());
            if let Some(left) = self.left.as_ref().and_then(|a| a.upgrade()) {
                store.split_node(left.id, self.offset)?;
                self.right = left.right.as_ref().and_then(|right| right.as_weak_item());
                self.index += self.offset;
                self.offset = 0;
            }
        }

        Ok(())
    }
}

pub(crate) trait ListType: AsInner<Inner = YTypeRef> {
    #[inline(always)]
    fn content_len(&self) -> u64 {
        self.as_inner().read().unwrap().len
    }

    fn iter_item(&self) -> ListIterator {
        let inner = self.as_inner().read().unwrap();
        ListIterator {
            next: inner.start.clone(),
        }
    }

    fn find_pos(&self, inner: &YType, index: u64) -> Option<ItemPosition> {
        let mut remaining = index;
        let start = inner.start().as_ref().map(Arc::downgrade);

        let mut pos = ItemPosition {
            parent: Arc::downgrade(self.as_inner()),
            left: None,
            right: start,
            index: 0,
            offset: 0,
        };

        if pos.right.is_none() {
            return Some(pos);
        }

        if let Some(markers) = &inner.markers {
            if let Some(marker) = markers.find_marker(inner, index) {
                if marker.index > remaining {
                    remaining = 0
                } else {
                    remaining -= marker.index;
                }
                pos.index = marker.index;
                pos.left = marker
                    .ptr
                    .upgrade()
                    .and_then(|i| i.left.as_ref().and_then(|i| i.as_weak_item()));
                pos.right = Some(marker.ptr);
            }
        };

        while remaining > 0 {
            if let Some(item) = &pos.right.and_then(|i| i.upgrade()) {
                if !item.deleted() {
                    let content_len = item.len();
                    if remaining < content_len {
                        pos.offset = remaining;
                        remaining = 0;
                    } else {
                        pos.index += content_len;
                        remaining -= content_len;
                    }
                }

                pos.left = Some(Arc::downgrade(item));
                pos.right = item.right.as_ref().and_then(|right| right.as_weak_item());
            } else {
                return None;
            }
        }

        Some(pos)
    }

    fn insert_at(&mut self, index: u64, content: Content) -> JwstCodecResult {
        if index > self.content_len() {
            return Err(JwstCodecError::IndexOutOfBound(index));
        }

        let inner = self.as_inner().write().unwrap();
        if let Some(pos) = self.find_pos(&inner, index) {
            if let Some(mut store) = inner.store_mut() {
                Self::insert_after(inner, &mut store, pos, content)?;
            }
        }

        Ok(())
    }

    fn insert_after(
        mut lock: RwLockWriteGuard<YType>,
        store: &mut DocStore,
        mut pos: ItemPosition,
        content: Content,
    ) -> JwstCodecResult {
        pos.normalize(store)?;

        let new_item_id = (store.client(), store.get_state(store.client())).into();

        if let Some(markers) = &lock.markers {
            markers.update_marker_changes(pos.index, content.clock_len() as i64);
        }

        let item = Arc::new(Item::new(
            new_item_id,
            content,
            pos.left.as_ref().and_then(|item| item.upgrade()),
            pos.right.as_ref().and_then(|item| item.upgrade()),
            Some(Parent::Type(pos.parent.clone())),
            None,
        ));

        match item.content.as_ref() {
            Content::Type(t) => {
                t.write().unwrap().set_item(item.clone());
            }
            Content::WeakType(t) => {
                t.upgrade().unwrap().write().unwrap().set_item(item.clone());
            }
            _ => {}
        }

        store.integrate(StructInfo::Item(item.clone()), 0, Some(&mut lock))?;

        pos.right = Some(Arc::downgrade(&item));
        pos.forward();

        Ok(())
    }

    fn get_item_at(&self, index: u64) -> Option<(ItemRef, u64)> {
        if index >= self.content_len() {
            return None;
        }

        let inner = self.as_inner().read().unwrap();

        if let Some(pos) = self.find_pos(&inner, index) {
            if pos.offset == 0 {
                return pos.right.and_then(|r| r.upgrade()).map(|r| (r, 0));
            } else {
                return pos.left.and_then(|r| r.upgrade()).map(|l| (l, pos.offset));
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

        let inner = self.as_inner().write().unwrap();
        if let Some(pos) = self.find_pos(&inner, idx) {
            if let Some(mut store) = inner.store_mut() {
                Self::remove_after(inner, &mut store, pos, len)?;
            }
        }

        Ok(())
    }

    fn remove_after(
        mut lock: RwLockWriteGuard<YType>,
        store: &mut DocStore,
        mut pos: ItemPosition,
        len: u64,
    ) -> JwstCodecResult {
        pos.normalize(store)?;
        let mut remaining = len as i64;

        while remaining > 0 {
            if let Some(item) = &pos.right.as_ref().and_then(|i| i.upgrade()) {
                if !item.deleted() {
                    let content_len = item.len() as i64;
                    if remaining < content_len {
                        store.split_node(item.id, remaining as u64)?;
                        remaining = 0;
                    } else {
                        remaining -= content_len;
                    }
                    store.delete_item(item, Some(&mut lock));
                }

                pos.forward();
            } else {
                break;
            }
        }

        if let Some(markers) = &lock.markers {
            markers.update_marker_changes(pos.index, -(len as i64) + remaining);
        }

        Ok(())
    }
}
