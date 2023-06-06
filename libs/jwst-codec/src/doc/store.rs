use super::*;
use crate::doc::StateVector;
use std::collections::VecDeque;
use std::sync::RwLockWriteGuard;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    ops::Range,
    sync::{Arc, RwLock, Weak},
};

#[derive(Default, Debug)]
pub struct DocStore {
    client: Client,
    pub items: Arc<RwLock<HashMap<Client, Vec<StructInfo>>>>,
    pub delete_set: Arc<RwLock<DeleteSet>>,

    // following fields are only used in memory
    pub types: Arc<RwLock<HashMap<String, YTypeRef>>>,
    pub pending: Option<Update>,
}

pub(crate) type StoreRef = Arc<RwLock<DocStore>>;
pub(crate) type WeakStoreRef = Weak<RwLock<DocStore>>;

impl PartialEq for DocStore {
    fn eq(&self, other: &Self) -> bool {
        self.client == other.client
    }
}

impl DocStore {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            ..Default::default()
        }
    }

    pub fn client(&self) -> Client {
        self.client
    }

    pub fn get_state(&self, client: Client) -> Clock {
        if let Some(structs) = self.items.read().unwrap().get(&client) {
            if let Some(last_struct) = structs.last() {
                last_struct.clock() + last_struct.len()
            } else {
                warn!("client {} has no struct info", client);
                0
            }
        } else {
            0
        }
    }

    pub fn get_state_vector(&self) -> StateVector {
        let mut state = StateVector::default();
        for (client, structs) in self.items.read().unwrap().iter() {
            if let Some(last_struct) = structs.last() {
                state.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        state
    }

    pub fn add_item(&self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client();
        match self.items.write().unwrap().entry(client_id) {
            Entry::Occupied(mut entry) => {
                let structs = entry.get_mut();
                if let Some(last_struct) = structs.last() {
                    let expect = last_struct.clock() + last_struct.len();
                    let actually = item.clock();
                    if expect != actually {
                        return Err(JwstCodecError::StructClockInvalid { expect, actually });
                    }
                } else {
                    warn!("client {} has no struct info", client_id);
                }
                structs.push(item);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![item]);
            }
        }

        Ok(())
    }

    /// binary search struct info on a sorted array
    pub fn get_item_index(items: &Vec<StructInfo>, clock: Clock) -> Option<usize> {
        let mut left = 0;
        let mut right = items.len() - 1;
        let middle = &items[right];
        let middle_clock = middle.clock();
        if middle_clock == clock {
            return Some(right);
        }
        let mut middle_index = (clock / (middle_clock + middle.len() - 1)) as usize * right;
        while left <= right {
            let middle = &items[middle_index];
            let middle_clock = middle.clock();
            if middle_clock <= clock {
                if clock < middle_clock + middle.len() {
                    return Some(middle_index);
                }
                left = middle_index + 1;
            } else {
                right = middle_index - 1;
            }
            middle_index = (left + right) / 2;
        }
        None
    }

    fn modify_item<F>(&mut self, id: Id, mut f: F)
    where
        F: FnMut(&mut Item),
    {
        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(idx) = Self::get_item_index(items, id.clock) {
                if let StructInfo::Item(item) = &mut items[idx] {
                    // SAFETY:
                    // we make sure store is the only entry of mutating an item,
                    // and we already hold mutable reference of store, so it's safe to do so
                    let item = unsafe { &mut *(Arc::as_ptr(item) as *mut Item) };
                    f(item);
                }
            }
        }
    }

    pub fn get_item<I: Into<Id>>(&self, id: I) -> Option<StructInfo> {
        self.get_item_with_idx(id).map(|(item, _)| item)
    }

    pub fn get_item_with_idx<I: Into<Id>>(&self, id: I) -> Option<(StructInfo, usize)> {
        let id = id.into();
        if let Some(items) = self.items.read().unwrap().get(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                return items.get(index).map(|item| (item.clone(), index));
            }
        }

        None
    }

    pub fn split_item<I: Into<Id>>(
        &mut self,
        id: I,
        diff: u64,
    ) -> JwstCodecResult<(StructInfo, StructInfo)> {
        debug_assert!(diff > 0);

        let id = id.into();

        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(idx) = Self::get_item_index(items, id.clock) {
                let item = items.get(idx).unwrap().clone();
                if item.is_item() {
                    let (left_item, right_item) = item.split_item(diff)?;
                    match (&item, &left_item) {
                        (StructInfo::Item(old_item), StructInfo::Item(new_item)) => {
                            // SAFETY:
                            // we make sure store is the only entry of mutating an item,
                            // and we already hold mutable reference of store, so it's safe to do so
                            let old_item = unsafe { &mut *(Arc::as_ptr(old_item) as *mut Item) };
                            let new_item = unsafe { &mut *(Arc::as_ptr(new_item) as *mut Item) };
                            std::mem::swap(old_item, new_item);
                        }
                        _ => {
                            items[idx] = left_item.clone();
                        }
                    }
                    items.insert(idx + 1, right_item.clone());
                    return Ok((item, right_item));
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn split_at_and_get_right<I: Into<Id>>(&self, id: I) -> JwstCodecResult<StructInfo> {
        let id = id.into();
        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                let item = items.get(index).unwrap().clone();
                let offset = id.clock - item.clock();
                if offset > 0 && item.is_item() {
                    let (left_item, right_item) = item.split_item(offset)?;
                    match (&item, &left_item) {
                        (StructInfo::Item(old_item), StructInfo::Item(new_item)) => {
                            // SAFETY:
                            // we make sure store is the only entry of mutating an item,
                            // and we already hold mutable reference of store, so it's safe to do so
                            let old_item = unsafe { &mut *(Arc::as_ptr(old_item) as *mut Item) };
                            let new_item = unsafe { &mut *(Arc::as_ptr(new_item) as *mut Item) };
                            std::mem::swap(old_item, new_item);
                        }
                        _ => {
                            items[index] = left_item.clone();
                        }
                    }
                    items.insert(index + 1, right_item.clone());

                    return Ok(right_item);
                } else {
                    return Ok(item);
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn split_at_and_get_left<I: Into<Id>>(&self, id: I) -> JwstCodecResult<StructInfo> {
        let id = id.into();
        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                let item = items.get(index).unwrap().clone();
                let offset = id.clock - item.clock();
                if offset != item.len() - 1 && !item.is_gc() {
                    let (left_item, right_item) = item.split_item(offset + 1)?;
                    items.insert(index + 1, right_item);
                    match (&item, &left_item) {
                        (StructInfo::Item(old_item), StructInfo::Item(new_item)) => {
                            // SAFETY:
                            // we make sure store is the only entry of mutating an item,
                            // and we already hold mutable reference of store, so it's safe to do so
                            let old_item = unsafe { &mut *(Arc::as_ptr(old_item) as *mut Item) };
                            let new_item = unsafe { &mut *(Arc::as_ptr(new_item) as *mut Item) };
                            std::mem::swap(old_item, new_item);
                            return Ok(item);
                        }
                        _ => {
                            items[index] = left_item.clone();
                            return Ok(left_item);
                        }
                    }
                } else {
                    return Ok(item);
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn self_check(&self) -> JwstCodecResult {
        for structs in self.items.read().unwrap().values() {
            for i in 1..structs.len() {
                let l = &structs[i - 1];
                let r = &structs[i];
                if l.clock() + l.len() != r.clock() {
                    return Err(JwstCodecError::StructSequenceInvalid {
                        client_id: l.client(),
                        clock: l.clock(),
                    });
                }
            }
        }

        Ok(())
    }

    // only for creating named type
    pub(crate) fn get_or_create_type(&self, name: &str) -> YTypeRef {
        match self.types.write().unwrap().entry(name.to_string()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let ty = YType {
                    kind: YTypeKind::Unknown,
                    root_name: Some(name.to_string()),
                    ..Default::default()
                }
                .into_ref();

                e.insert(ty.clone());

                ty
            }
        }
    }

    pub(crate) fn get_start_item(&self, s: &StructInfo) -> StructInfo {
        let mut o = s.clone();

        while let Some(left_id) = o.left_id() {
            if let Some(left_struct) = self.get_item(left_id) {
                if left_struct.is_item() {
                    o = left_struct;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        o
    }

    pub(crate) fn repair(&self, item: &mut Item, store_ref: StoreRef) -> JwstCodecResult {
        if let Some(left_id) = item.left_id {
            let left = self.split_at_and_get_left(left_id)?;
            item.left_id = Some(left.id());
        }

        if let Some(right_id) = item.right_id {
            let right = self.split_at_and_get_right(right_id)?;
            item.right_id = Some(right.id());
        }

        match &item.parent {
            // root level named type
            // doc.get_or_create_text(Some("content"))
            //                              ^^^^^^^ Parent::String("content")
            Some(Parent::String(str)) => {
                let ty = self.get_or_create_type(str);
                ty.write().unwrap().store = Arc::downgrade(&store_ref);
                item.parent.replace(Parent::Type(ty));
            }
            // type as item
            // let text = doc.create_text("text")
            // Item { id: (1, 0), content: Content::Type(YText) }
            //        ^^ Parent::Id((1, 0))
            Some(Parent::Id(parent_id)) => {
                match self.get_item(*parent_id) {
                    Some(StructInfo::Item(_)) => match &item.content.as_ref() {
                        Content::Type(ty) => {
                            item.parent.replace(Parent::Type(ty.clone()));
                        }
                        Content::Deleted(_) => {
                            // parent got deleted, take it.
                            item.parent.take();
                        }
                        _ => {
                            return Err(JwstCodecError::InvalidParent);
                        }
                    },
                    _ => {
                        // GC & Skip are not valid parent, take it.
                        item.parent.take();
                    }
                }
            }
            // no item.parent, borrow left.parent or right.parent
            None => {
                if let Some(left) = item.left_id.and_then(|left_id| self.get_item(left_id)) {
                    item.parent = left.parent().cloned();
                    item.parent_sub = left.parent_sub().cloned();
                } else if let Some(right) =
                    item.right_id.and_then(|right_id| self.get_item(right_id))
                {
                    item.parent = right.parent().cloned();
                    item.parent_sub = right.parent_sub().cloned();
                }
            }
            _ => {}
        };

        Ok(())
    }

    pub(crate) fn integrate_struct_info(
        &mut self,
        mut struct_info: StructInfo,
        offset: u64,
        parent: Option<RwLockWriteGuard<YType>>,
    ) -> JwstCodecResult {
        match &mut struct_info {
            StructInfo::Item(item) => {
                debug_assert_eq!(Arc::strong_count(item), 1);
                // SAFETY:
                // before we integrate struct into store,
                // the struct => Arc<Item> is owned reference actually,
                // no one else refer to such item yet, we can safely mutable refer to it now.
                let item = unsafe { &mut *(Arc::as_ptr(item) as *mut Item) };

                if offset > 0 {
                    item.id.clock += offset;
                    let left =
                        self.split_at_and_get_left(Id::new(item.id.client, item.id.clock - 1))?;
                    item.left_id = Some(Id::new(item.id.client, left.clock() + left.len() - 1));
                    item.content = Arc::new(item.content.split(offset)?.1);
                }

                let id = item.id;

                let mut left = item.left_id.and_then(|left_id| self.get_item(left_id));
                let mut right = item.right_id.and_then(|right_id| self.get_item(right_id));

                let parent = parent.or_else(|| {
                    if let Some(parent) = &item.parent {
                        match parent {
                            Parent::Type(ty) => Some(ty.write().unwrap()),
                            _ => {
                                // we've already recovered all `item.parent` to `Parent::Type` branch in [repair] phase
                                unreachable!()
                            }
                        }
                    } else {
                        None
                    }
                });

                if let Some(mut parent) = parent {
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
                            left.right_id().and_then(|right_id| self.get_item(right_id))
                        } else if let Some(parent_sub) = &item.parent_sub {
                            let o = parent.map.as_ref().and_then(|m| m.get(parent_sub).cloned());
                            o.as_ref().map(|o| self.get_start_item(o))
                        } else {
                            parent.start.clone()
                        };

                        let mut conflicting_items = HashSet::new();
                        let mut items_before_origin = HashSet::new();

                        while let Some(conflict) = &o {
                            if Some(conflict) == right.as_ref() {
                                break;
                            }

                            items_before_origin.insert(conflict.id());
                            conflicting_items.insert(conflict.id());
                            match conflict {
                                StructInfo::Item(c) => {
                                    let conflict_id = item.id;
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
                                        Some(right_id) => self.get_item(right_id),
                                        None => None,
                                    };
                                }
                                _ => {
                                    break;
                                }
                            }
                        }
                    }

                    // now manipulating the store, so we gonna acquire the write lock.
                    // reconnect left/right
                    match &left {
                        // has left, connect left <-> self <-> left.right
                        Some(left) => {
                            item.left_id = Some(left.id());
                            self.modify_item(left.id(), |left| {
                                item.right_id = left.right_id.replace(id);
                            });
                        }
                        // no left, right = parent.start
                        None => {
                            right = if let Some(parent_sub) = &item.parent_sub {
                                parent
                                    .map
                                    .as_ref()
                                    .and_then(|map| map.get(parent_sub))
                                    .map(|r| self.get_start_item(r))
                            } else {
                                parent.start.replace(struct_info.clone())
                            };
                        }
                    }

                    match right {
                        // has right, connect
                        Some(right) => {
                            item.right_id = Some(right.id());
                            self.modify_item(right.id(), |right| {
                                item.left_id = right.left_id.replace(id);
                            });
                        }

                        // no right, parent.start = item, delete item.left
                        None => {
                            if let Some(parent_sub) = &item.parent_sub {
                                parent
                                    .map
                                    .get_or_insert(HashMap::default())
                                    .insert(parent_sub.to_string(), struct_info.clone());

                                if let Some(left) = &left {
                                    self.delete(left.id(), left.len());
                                    left.delete();
                                }
                            }
                        }
                    }

                    // should delete
                    if parent
                        .item
                        .as_ref()
                        .and_then(|item| item.upgrade())
                        .map(|item| item.deleted())
                        .unwrap_or(false)
                        || item.parent_sub.is_some() && item.right_id.is_some()
                    {
                        self.delete(id, item.len());
                        item.delete();
                    } else {
                        // adjust parent length
                        if item.parent_sub.is_none() && item.countable() {
                            parent.content_len += item.len();
                            parent.item_len += 1;
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
        self.add_item(struct_info)
    }

    pub(crate) fn delete(&self, id: Id, len: u64) {
        let mut delete_set = self.delete_set.write().unwrap();
        delete_set.add(id.client, id.clock, len);
    }

    pub(crate) fn delete_range(&mut self, client: u64, range: Range<u64>) -> JwstCodecResult {
        let start = range.start;
        let end = range.end;
        if let Some(items) = self.items.write().unwrap().get_mut(&client) {
            if let Some(mut idx) = DocStore::get_item_index(items, start) {
                {
                    // id.clock <= range.start < id.end
                    // need to split the item and delete the right part
                    // -----item-----
                    //    ^start
                    let struct_info = &items[idx];
                    let id = struct_info.id();

                    if !struct_info.deleted() && id.clock < range.start {
                        match struct_info.split_item(start - id.clock) {
                            Ok((left, right)) => {
                                items[idx] = left;
                                items.insert(idx + 1, right);
                                idx += 1;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                };

                while idx < items.len() {
                    let mut struct_info = &items[idx];
                    let id = struct_info.id();

                    if !struct_info.deleted() && id.clock < end {
                        // need to split the item
                        // -----item-----
                        //           ^end
                        if end < id.clock + struct_info.len() {
                            match struct_info.split_item(end - id.clock) {
                                Ok((left, right)) => {
                                    items[idx] = left;
                                    items.insert(idx + 1, right);
                                    struct_info = &items[idx];
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        self.delete(id, struct_info.len());
                        struct_info.delete();
                    } else {
                        break;
                    }
                    idx += 1;
                }
            }
        }

        Ok(())
    }

    fn diff_state_vectors(
        local_state_vector: &StateVector,
        remote_state_vector: &StateVector,
    ) -> Vec<(Client, Clock)> {
        let mut diff = Vec::new();

        for (client, &remote_clock) in remote_state_vector.iter() {
            let local_clock = local_state_vector.get(client);
            if local_clock > remote_clock {
                diff.push((*client, remote_clock));
            }
        }

        for (client, _) in local_state_vector.iter() {
            if remote_state_vector.get(client) == 0 {
                diff.push((*client, 0));
            }
        }

        diff
    }

    pub fn encode_with_state_vector<W: CrdtWriter>(
        &self,
        sv: &StateVector,
        encoder: &mut W,
    ) -> JwstCodecResult {
        let local_state_vector = self.get_state_vector();
        let diff = Self::diff_state_vectors(&local_state_vector, sv);
        let mut update_structs: HashMap<u64, VecDeque<StructInfo>> = HashMap::new();

        for (client, clock) in diff {
            // We have made sure that the client is in the local state vector in diff_state_vectors()
            if let Some(items) = self.items.read().unwrap().get(&client) {
                if items.is_empty() {
                    continue;
                }

                update_structs.insert(client, VecDeque::new());
                let vec_struct_info = update_structs.get_mut(&client).unwrap();

                // the smallest clock in items may exceed the clock
                let clock = items.first().unwrap().id().clock.max(clock);
                if let Some(index) = Self::get_item_index(items, clock) {
                    let first_block = items.get(index).unwrap();
                    let offset = first_block.clock() - clock;
                    if offset != 0 {
                        // needs to implement Content split first
                        vec_struct_info
                            .push_back(first_block.clone().split_item(offset)?.clone().1);
                    } else {
                        vec_struct_info.push_back(first_block.clone());
                    }

                    for item in items.iter().skip(index + 1) {
                        vec_struct_info.push_back(item.clone());
                    }
                }
            }
        }

        let update = Update {
            structs: update_structs,
            delete_set: self.delete_set.read().unwrap().clone(),
            ..Update::default()
        };

        update.write(encoder)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_state() {
        {
            let doc_store = DocStore::new();
            let state = doc_store.get_state(1);
            assert_eq!(state, 0);
        }

        {
            let doc_store = DocStore::new();

            let client_id = 1;

            let struct_info1 = StructInfo::GC {
                id: Id::new(1, 1),
                len: 5,
            };
            let struct_info2 = StructInfo::Skip {
                id: Id::new(1, 6),
                len: 7,
            };

            doc_store
                .items
                .write()
                .unwrap()
                .insert(client_id, vec![struct_info1, struct_info2.clone()]);

            let state = doc_store.get_state(client_id);

            assert_eq!(state, struct_info2.clock() + struct_info2.len());

            assert!(doc_store.self_check().is_ok());
        }
    }

    #[test]
    fn test_get_state_vector() {
        {
            let doc_store = DocStore::new();
            let state_map = doc_store.get_state_vector();
            assert!(state_map.is_empty());
        }

        {
            let doc_store = DocStore::new();

            let client1 = 1;
            let struct_info1 = StructInfo::GC {
                id: (1, 0).into(),
                len: 5,
            };

            let client2 = 2;
            let struct_info2 = StructInfo::GC {
                id: (2, 0).into(),
                len: 6,
            };
            let struct_info3 = StructInfo::Skip {
                id: (2, 6).into(),
                len: 1,
            };

            doc_store
                .items
                .write()
                .unwrap()
                .insert(client1, vec![struct_info1.clone()]);
            doc_store
                .items
                .write()
                .unwrap()
                .insert(client2, vec![struct_info2, struct_info3.clone()]);

            let state_map = doc_store.get_state_vector();

            assert_eq!(
                state_map.get(&client1),
                struct_info1.clock() + struct_info1.len()
            );
            assert_eq!(
                state_map.get(&client2),
                struct_info3.clock() + struct_info3.len()
            );

            assert!(doc_store.self_check().is_ok());
        }
    }

    #[test]
    fn test_add_item() {
        let doc_store = DocStore::new();

        let struct_info1 = StructInfo::GC {
            id: Id::new(1, 0),
            len: 5,
        };
        let struct_info2 = StructInfo::Skip {
            id: Id::new(1, 5),
            len: 1,
        };
        let struct_info3_err = StructInfo::Skip {
            id: Id::new(1, 5),
            len: 1,
        };
        let struct_info3 = StructInfo::Skip {
            id: Id::new(1, 6),
            len: 1,
        };

        assert!(doc_store.add_item(struct_info1.clone()).is_ok());
        assert!(doc_store.add_item(struct_info2).is_ok());
        assert_eq!(
            doc_store.add_item(struct_info3_err),
            Err(JwstCodecError::StructClockInvalid {
                expect: 6,
                actually: 5
            })
        );
        assert!(doc_store.add_item(struct_info3.clone()).is_ok());
        assert_eq!(
            doc_store.get_state(struct_info1.client()),
            struct_info3.clock() + struct_info3.len()
        );
    }

    #[test]
    fn test_get_item() {
        {
            let doc_store = DocStore::new();
            let struct_info = StructInfo::GC {
                id: Id::new(1, 0),
                len: 10,
            };
            doc_store.add_item(struct_info.clone()).unwrap();

            assert_eq!(doc_store.get_item(Id::new(1, 9)), Some(struct_info));
        }

        {
            let doc_store = DocStore::new();
            let struct_info1 = StructInfo::GC {
                id: Id::new(1, 0),
                len: 10,
            };
            let struct_info2 = StructInfo::GC {
                id: Id::new(1, 10),
                len: 20,
            };
            doc_store.add_item(struct_info1).unwrap();
            doc_store.add_item(struct_info2.clone()).unwrap();

            assert_eq!(doc_store.get_item(Id::new(1, 25)), Some(struct_info2));
        }

        {
            let doc_store = DocStore::new();

            assert_eq!(doc_store.get_item(Id::new(1, 0)), None);
        }

        {
            let doc_store = DocStore::new();
            let struct_info1 = StructInfo::GC {
                id: Id::new(1, 0),
                len: 10,
            };
            let struct_info2 = StructInfo::GC {
                id: Id::new(1, 10),
                len: 20,
            };
            doc_store.add_item(struct_info1).unwrap();
            doc_store.add_item(struct_info2).unwrap();

            assert_eq!(doc_store.get_item(Id::new(1, 35)), None);
        }
    }

    #[test]
    fn test_split_and_get() {
        let doc_store = DocStore::new();
        let struct_info1 = StructInfo::Item(Arc::new(Item {
            id: (1, 0).into(),
            content: Arc::new(Content::String(String::from("octo"))),
            ..Default::default()
        }));

        let struct_info2 = StructInfo::Item(Arc::new(Item {
            id: Id::new(1, struct_info1.len()),
            left_id: Some(struct_info1.id()),
            content: Arc::new(Content::String(String::from("base"))),
            ..Default::default()
        }));
        doc_store.add_item(struct_info1.clone()).unwrap();
        doc_store.add_item(struct_info2).unwrap();

        let s1 = doc_store.get_item(Id::new(1, 0)).unwrap();
        assert_eq!(s1, struct_info1);
        let left = doc_store.split_at_and_get_left((1, 1)).unwrap();
        assert_eq!(left.len(), 2); // octo => oc_to

        // s1 used to be (1, 4), but it actually ref of first item in store, so now it should be (1, 2)
        assert_eq!(
            s1, left,
            "doc internal mutation should not modify the pointer"
        );
        let right = doc_store.split_at_and_get_right((1, 5)).unwrap();
        assert_eq!(right.len(), 3); // base => b_ase
    }
}
