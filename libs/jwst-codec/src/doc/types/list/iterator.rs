use super::*;

pub struct ListIterator {
    pub(super) store: DocStore,
    pub(super) next: Option<StructRef>,
    pub(super) content: Option<Content>,
    pub(super) content_idx: u64,
}

impl ListIterator {
    pub fn new(store: DocStore, root: TypeStoreRef) -> ListIterator {
        Self {
            store: store.clone(),
            next: root.borrow().start.clone(),
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
                        return Some(Err(JwstCodecError::InvalidContentIndex(self.content_idx)))
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

impl Iterator for ListIterator {
    type Item = JwstCodecResult<Content>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.next.clone() {
            if n.deleted() {
                self.next = self.store.get_item(n.right_id()?).ok();
                continue;
            }

            if let Some(content) = self.next_content() {
                return Some(content);
            } else {
                if let Some(item) = n.as_item() {
                    self.content = Some(item.content);
                    self.content_idx = 0;
                    if let Some(right_id) = item.right_id {
                        self.next = self.store.get_item(right_id).ok();
                    } else {
                        self.next = None;
                    }
                } else {
                    return Some(Err(JwstCodecError::InvalidStructType(n.clone())));
                }
            }
        }

        self.next_content()
    }
}
