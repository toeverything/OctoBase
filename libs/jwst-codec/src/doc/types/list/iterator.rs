use super::*;

pub struct ListIterator {
    pub(super) next: Option<StructInfo>,
}

impl Iterator for ListIterator {
    type Item = ItemRef;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(StructInfo::Item(item)) = self.next.take() {
            self.next = item.right.clone();

            if item.deleted() {
                continue;
            }

            return Some(item);
        }

        None
    }
}
