use super::*;

pub(crate) struct ListIterator {
    pub(super) cur: Somr<Item>,
}

impl Iterator for ListIterator {
    type Item = Somr<Item>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.cur.clone().get() {
            let cur = std::mem::replace(&mut self.cur, item.right.clone());
            if item.deleted() {
                continue;
            }

            return Some(cur);
        }

        None
    }
}

impl ListIterator {
    pub fn new(start: Somr<Item>) -> Self {
        Self { cur: start }
    }
}
