use super::*;

pub struct ListIterator {
    pub(super) next: Option<StructInfo>,
}

impl Iterator for ListIterator {
    type Item = ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(StructInfo::Item(item)) = self.next.take().map(|s| {
            if let StructInfo::WeakItem(item) = s {
                StructInfo::Item(item.upgrade().unwrap())
            } else {
                s
            }
        }) {
            self.next = item.right.clone();

            if item.deleted() {
                continue;
            }

            return Some(item);
        }

        None
    }
}
