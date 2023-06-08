use super::*;
use std::sync::RwLockReadGuard;

pub struct ListIterator<'a> {
    pub(super) store: RwLockReadGuard<'a, DocStore>,
    pub(super) next: Option<StructInfo>,
}

impl Iterator for ListIterator<'_> {
    type Item = ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.next.take() {
            self.next = n
                .right_id()
                .and_then(|right_id| self.store.get_node(right_id));

            if n.deleted() {
                continue;
            }

            return n.as_item();
        }

        None
    }
}
