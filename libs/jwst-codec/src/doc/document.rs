use super::*;
use std::{
    cell::RefCell,
    collections::HashSet,
    ops::{Deref, DerefMut, Range},
    sync::Arc,
};

pub struct Doc {
    // TODO: use function in code
    #[allow(dead_code)]
    // random client id for each doc
    client_id: u64,
    // TODO: use function in code
    #[allow(dead_code)]
    // random id for each doc, use in sub doc
    guid: String,
    // root_type: HashMap<String, Item>,
    store: DocStore,
}

impl Default for Doc {
    fn default() -> Self {
        Self {
            client_id: rand::random(),
            guid: nanoid!(),
            // share: HashMap::new(),
            store: DocStore::new(),
        }
    }
}

impl Doc {
    pub fn apply_update(&mut self, mut update: Update) -> JwstCodecResult {
        let mut retry = false;
        loop {
            for (s, offset) in update.iter(self.store.get_state_vector()) {
                self.integrate_struct_info(s, offset)?;
            }

            for (client, range) in update.delete_set_iter(self.store.get_state_vector()) {
                self.delete_range(client, range)?;
            }

            if let Some(mut pending_update) = self.store.pending.take() {
                if pending_update
                    .missing_state
                    .iter()
                    .any(|(client, clock)| self.store.get_state(*client) > *clock)
                {
                    // new update has been applied to the doc, need to re-integrate
                    retry = true;
                }

                for (client, range) in pending_update.delete_set_iter(self.store.get_state_vector())
                {
                    self.delete_range(client, range)?;
                }

                if update.is_pending_empty() {
                    update = pending_update;
                } else {
                    // drain all pending state to pending update for later iteration
                    update.drain_pending_state();
                    update = Update::merge([pending_update, update]);
                }
            } else {
                // no pending update at store

                // no pending update in current iteration
                // thank god, all clean
                if update.is_pending_empty() {
                    break;
                } else {
                    // need to turn all pending state into update for later iteration
                    update.drain_pending_state();
                };
            }

            // can't integrate any more, save the pending update
            if !retry {
                if !update.is_pending_empty() {
                    self.store.pending = Some(update)
                }
                break;
            }
        }

        Ok(())
    }

    fn integrate_struct_info(
        &mut self,
        mut struct_info: StructInfo,
        offset: u64,
    ) -> JwstCodecResult<()> {
        self.repair(&mut struct_info)?;
        let struct_info = Arc::new(RefCell::new(struct_info));
        match struct_info.borrow_mut().deref_mut() {
            StructInfo::Item { id, item } => {
                let mut left =
                    item.left_id
                        .and_then(|left_id| match self.store.get_item(left_id) {
                            Ok(left) => Some(left),
                            _ => None,
                        });

                let mut right =
                    item.right_id
                        .and_then(|right_id| match self.store.get_item(right_id) {
                            Ok(right) => Some(right),
                            _ => None,
                        });

                let parent = if let Some(parent) = &item.parent {
                    match parent {
                        Parent::String(name) => Some(self.store.get_or_create_type(name)),
                        Parent::Id(id) => {
                            if let Some(item) = self.store.get_item(*id)?.borrow().item() {
                                match &item.content {
                                    Content::Type(ytype) => {
                                        Some(Arc::new(RefCell::new(ytype.clone().into())))
                                    }
                                    _ => {
                                        return Err(JwstCodecError::InvalidParent);
                                    }
                                }
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                };

                if offset > 0 {
                    id.clock += offset;
                    left = Some(
                        self.store
                            .get_item_clean_end(Id::new(id.client, id.clock - 1))?,
                    );
                    item.content.split(offset)?;
                }

                if let Some(parent) = parent {
                    let mut parent = parent.borrow_mut();
                    let right_is_null_or_has_left = match &right {
                        None => true,
                        Some(right) => right.left_id().is_some(),
                    };
                    let left_has_other_right_than_self = match &left {
                        Some(left) => left.right_id().is_some(),
                        _ => false,
                    };

                    // conflicts
                    if (left.is_none() && right_is_null_or_has_left)
                        || left_has_other_right_than_self
                    {
                        // set the first conflicting item
                        let mut o = if let Some(left) = left.clone() {
                            left.right_id().and_then(|right_id| {
                                match self.store.get_item(right_id) {
                                    Ok(item) => Some(item),
                                    Err(_) => None,
                                }
                            })
                        } else if let Some(parent_sub) = &item.parent_sub {
                            let o = parent.map.get(parent_sub).cloned();
                            if let Some(o) = o.as_ref() {
                                Some(self.store.get_start_item(o.clone()))
                            } else {
                                None
                            }
                        } else {
                            parent.start.clone()
                        };

                        let mut conflicting_items = HashSet::new();
                        let mut items_before_origin = HashSet::new();

                        while let Some(conflict) = o {
                            if Some(conflict.clone()) == right {
                                break;
                            }

                            items_before_origin.insert(conflict.id());
                            conflicting_items.insert(conflict.id());
                            match conflict.borrow().deref() {
                                StructInfo::Item {
                                    id: conflict_id,
                                    item: c,
                                } => {
                                    if item.left_id == c.left_id {
                                        // case 1
                                        if conflict_id.client < id.client {
                                            left = Some(conflict.clone());
                                            conflicting_items.clear();
                                        } else if item.right_id == c.right_id {
                                            // `this` and `c` are conflicting and point to the same
                                            // integration points. The id decides which item comes first.
                                            // Since `this` is to the left of `c`, we can break here.
                                            break;
                                        }
                                    } else if let Some(conflict_item_left) = c.left_id {
                                        if items_before_origin.contains(&conflict_item_left)
                                            && !conflicting_items.contains(&conflict_item_left)
                                        {
                                            left = Some(conflict.clone());
                                            conflicting_items.clear();
                                        }
                                    } else {
                                        break;
                                    }
                                    o = match c.right_id {
                                        Some(right_id) => Some(self.store.get_item(right_id)?),
                                        None => None,
                                    };
                                }
                                _ => {
                                    break;
                                }
                            }
                        }

                        // reconnect left/right
                        match left.clone() {
                            // has left, connect left <-> self <-> left.right
                            Some(left) => {
                                item.left_id = Some(left.id());
                                left.modify(|left| {
                                    if let StructInfo::Item { item: left, .. } = left {
                                        left.right_id = left.right_id.replace(*id);
                                    }
                                });
                            }
                            // no left, right = parent.start
                            None => {
                                right = if let Some(parent_sub) = &item.parent_sub {
                                    if let Some(r) = parent.map.get(parent_sub).cloned() {
                                        Some(self.store.get_start_item(r))
                                    } else {
                                        None
                                    }
                                } else {
                                    parent.start.clone()
                                };
                            }
                        }

                        match right {
                            // has right, connect
                            Some(right) => right.modify(|right| {
                                if let StructInfo::Item { item: right, .. } = right {
                                    right.left_id = right.left_id.replace(*id);
                                }
                                item.right_id = Some(right.id());
                            }),
                            // no right, parent.start = item, delete item.left
                            None => {
                                if let Some(parent_sub) = &item.parent_sub {
                                    parent
                                        .map
                                        .insert(parent_sub.to_string(), struct_info.clone().into());
                                }
                                if let Some(left) = &left {
                                    self.store.delete(left.id(), left.len());
                                    left.as_mut_ref().delete();
                                }
                            }
                        }

                        // should delete
                        if parent.item.is_some() && parent.item.clone().unwrap().deleted()
                            || item.parent_sub.is_some() && item.right_id.is_some()
                        {
                            self.store.delete(*id, item.len());
                            item.delete();
                        } else {
                            // adjust parent length
                            if item.parent_sub.is_none() && item.flags.countable() {
                                parent.len += item.len();
                            }
                        }
                    }
                }
            }
            StructInfo::GC { id, len } => {
                if offset > 0 {
                    id.clock += offset;
                    *len -= offset;
                }
            }
            StructInfo::Skip { .. } => {
                // skip ignored
            }
        }
        self.store.add_item_ref(struct_info.into())
    }

    fn repair(&mut self, info: &mut StructInfo) -> JwstCodecResult {
        if let StructInfo::Item { item, .. } = info {
            if let Some(left_id) = item.left_id {
                let left = self.store.get_item_clean_end(left_id)?;
                item.left_id = Some(left.id());
            }

            if let Some(right_id) = item.right_id {
                let right = self.store.get_item_clean_start(right_id)?;
                item.right_id = Some(right.id());
            }

            match &item.parent {
                // root level named type
                Some(Parent::String(str)) => {
                    self.store.get_or_create_type(str);
                }
                // type as item
                Some(Parent::Id(parent_id)) => {
                    let parent = self.store.get_item(*parent_id)?;
                    match parent.borrow().deref() {
                        StructInfo::Item { .. } => match &item.content {
                            Content::Type(_) => {
                                // do nothing but catch the variant.
                            }
                            // parent got deleted, take it.
                            Content::Deleted(_) => {
                                item.parent.take();
                            }
                            _ => {
                                return Err(JwstCodecError::InvalidParent);
                            }
                        },
                        // GC & Skip are not valid parent, take it.
                        _ => {
                            item.parent.take();
                        }
                    }
                    if !parent.is_item() {
                        item.parent.take();
                    }
                }
                // no item.parent, borrow left.parent or right.parent
                None => {
                    if let Some(left_id) = item.left_id {
                        let left = self.store.get_item(left_id)?;
                        item.parent = left.borrow().parent().cloned();
                        item.parent_sub = left.borrow().parent_sub().cloned();
                    } else if let Some(right_id) = item.right_id {
                        let right = self.store.get_item(right_id)?;
                        item.parent = right.borrow().parent().cloned();
                        item.parent_sub = right.borrow().parent_sub().cloned();
                    }
                }
            }
        }

        Ok(())
    }

    fn delete_range(&mut self, client: u64, range: Range<u64>) -> JwstCodecResult {
        let start = range.start;
        let end = range.end;
        self.store.with_client_items_mut(client, |items| {
            if let Some(mut idx) = DocStore::get_item_index(items, start) {
                let right = {
                    let mut struct_info = items[idx].borrow_mut();
                    let id = struct_info.id();

                    // id.clock <= range.start < id.end
                    // need to split the item and delete the right part
                    if !struct_info.flags().deleted() && id.clock < range.start {
                        match struct_info.split_item(start - id.clock) {
                            Ok(r) => Some(r),
                            Err(e) => return Err(e),
                        }
                    } else {
                        None
                    }
                };

                if let Some(right) = right {
                    items.insert(idx + 1, right.into());
                    idx += 1;
                }

                while idx < items.len() {
                    let right = {
                        let mut struct_info = items[idx].borrow_mut();
                        let id = struct_info.id();

                        if !struct_info.flags().deleted() && id.clock < end {
                            let right = if end < id.clock + struct_info.len() {
                                match struct_info.split_item(end - id.clock) {
                                    Ok(r) => Some(r),
                                    Err(e) => return Err(e),
                                }
                            } else {
                                None
                            };
                            self.store.delete(id, struct_info.len());
                            struct_info.delete();
                            right
                        } else {
                            break;
                        }
                    };
                    if let Some(right) = right {
                        items.insert(idx + 1, right.into());
                    }
                    idx += 1;
                }
            }

            Ok(())
        })
    }
}
