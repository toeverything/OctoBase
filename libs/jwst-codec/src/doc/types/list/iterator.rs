use super::*;
use std::sync::RwLockReadGuard;

pub struct ListIterator<'a> {
    pub(super) store: RwLockReadGuard<'a, DocStore>,
    pub(super) next: Option<StructInfo>,
    pub(super) content: Option<Arc<Content>>,
    pub(super) content_idx: u64,
}

impl<'a> ListIterator<'a> {
    pub fn new(store: RwLockReadGuard<'a, DocStore>, root: StructInfo) -> ListIterator {
        Self {
            store,
            next: Some(root),
            content: None,
            content_idx: 0,
        }
    }

    pub fn next_content(&mut self) -> Option<JwstCodecResult<Content>> {
        if let Some(content) = self.content.as_mut() {
            if self.content_idx < content.clock_len() {
                match content.at(self.content_idx) {
                    Ok(Some(value)) => {
                        self.content_idx += 1;
                        if self.content_idx == content.clock_len() {
                            self.content = None;
                        }
                        return Some(Ok(value));
                    }
                    Ok(None) => {
                        return Some(Err(JwstCodecError::IndexOutOfBound(self.content_idx)))
                    }
                    Err(e) => return Some(Err(e)),
                }
            }
            unreachable!("content_idx should never be greater than content.clock_len()")
        } else {
            None
        }
    }
}

impl Iterator for ListIterator<'_> {
    type Item = JwstCodecResult<Content>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.next.clone() {
            if n.deleted() {
                self.next = self.store.get_item(n.right_id()?);
                continue;
            }

            if let Some(content) = self.next_content() {
                return Some(content);
            } else if let Some(item) = n.as_item() {
                self.content = Some(item.content.clone());
                self.content_idx = 0;
                if let Some(right_id) = item.right_id {
                    self.next = self.store.get_item(right_id);
                } else {
                    self.next = None;
                }
            } else {
                return Some(Err(JwstCodecError::InvalidStructType(n)));
            }
        }

        self.next_content()
    }
}
