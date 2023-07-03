use super::*;
use crate::{
    doc::StateVector,
    sync::{Arc, RwLock, RwLockWriteGuard, Weak},
};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    ops::Range,
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

    pub fn get_item<I: Into<Id>>(&self, id: I) -> Option<Arc<Item>> {
        self.get_node_with_idx(id)
            .and_then(|(item, _)| item.as_item())
    }

    pub fn get_node<I: Into<Id>>(&self, id: I) -> Option<StructInfo> {
        self.get_node_with_idx(id).map(|(item, _)| item)
    }

    pub fn get_node_with_idx<I: Into<Id>>(&self, id: I) -> Option<(StructInfo, usize)> {
        let id = id.into();
        if let Some(items) = self.items.read().unwrap().get(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                return items.get(index).map(|item| (item.clone(), index));
            }
        }

        None
    }

    pub fn split_node<I: Into<Id>>(&mut self, id: I, diff: u64) -> JwstCodecResult {
        debug_assert!(diff > 0);

        let id = id.into();

        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(idx) = Self::get_item_index(items, id.clock) {
                Self::split_node_at(items, idx, diff)?;

                return Ok(());
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn split_node_at(
        items: &mut Vec<StructInfo>,
        idx: usize,
        diff: u64,
    ) -> JwstCodecResult<(StructInfo, StructInfo)> {
        debug_assert!(diff > 0);

        let node = items.get(idx).unwrap().clone();
        debug_assert!(node.is_item());
        let (left_node, right_node) = node.split_at(diff)?;
        match (&node, &left_node, &right_node) {
            (
                StructInfo::Item(raw_left_item),
                StructInfo::Item(new_left_item),
                StructInfo::Item(right_item),
            ) => {
                // SAFETY:
                // we make sure store is the only entry of mutating an item,
                // and we already hold mutable reference of store, so it's safe to do so
                unsafe {
                    let left_item = Item::inner_mut(raw_left_item);
                    let right_item = Item::inner_mut(right_item);
                    left_item.content = new_left_item.content.clone();

                    // we had the correct left/right content
                    // now build the references
                    if let Some(right_right) = &left_item.right.as_ref().and_then(|i| i.as_item()) {
                        Item::inner_mut(right_right).left = Some(right_node.as_weak());
                        right_item.right = Some(StructInfo::WeakItem(Arc::downgrade(right_right)));
                    }
                    left_item.right = Some(right_node.as_weak());

                    right_item.origin_left_id = Some(left_item.last_id());
                    right_item.left = Some(node.as_weak());
                    right_item.origin_right_id = left_item.origin_right_id;
                };
            }
            _ => {
                items[idx] = left_node;
            }
        }
        items.insert(idx + 1, right_node.clone());

        Ok((node, right_node))
    }

    pub fn split_at_and_get_right<I: Into<Id>>(&self, id: I) -> JwstCodecResult<StructInfo> {
        let id = id.into();
        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                let item = items.get(index).unwrap().clone();
                let offset = id.clock - item.clock();
                if offset > 0 && item.is_item() {
                    let (_, right) = Self::split_node_at(items, index, offset)?;
                    return Ok(right);
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
                    let (left, _) = Self::split_node_at(items, index, offset + 1)?;
                    return Ok(left);
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
                let ty = Arc::new(RwLock::new(YType {
                    kind: YTypeKind::Unknown,
                    root_name: Some(name.to_string()),
                    ..Default::default()
                }));

                e.insert(ty.clone());

                ty
            }
        }
    }

    /// A repair for an item do such things:
    ///   - split left if needed  (insert in between a splitable item)
    ///   - split right if needed (insert in between a splitable item)
    ///   - recover parent to [Parent::Type]
    ///     - [Parent::String] for root level named type (e.g `doc.get_or_create_text("content")`)
    ///     - [Parent::Id] for type as item (e.g `doc.create_text()`)
    ///     - [None] means borrow left.parent or right.parent
    pub(crate) fn repair(&self, item: &mut Item, store_ref: StoreRef) -> JwstCodecResult {
        if let Some(left_id) = item.origin_left_id {
            item.left = Some(self.split_at_and_get_left(left_id)?.as_weak());
        }

        if let Some(right_id) = item.origin_right_id {
            item.right = Some(self.split_at_and_get_right(right_id)?.as_weak());
        }

        match &item.parent {
            // root level named type
            // doc.get_or_create_text(Some("content"))
            //                              ^^^^^^^ Parent::String("content")
            Some(Parent::String(str)) => {
                let ty = self.get_or_create_type(str);
                ty.write().unwrap().store = Arc::downgrade(&store_ref);
                item.parent.replace(Parent::Type(Arc::downgrade(&ty)));
            }
            // type as item
            // let text = doc.create_text("text")
            // Item { id: (1, 0), content: Content::Type(YText) }
            //        ^^ Parent::Id((1, 0))
            Some(Parent::Id(parent_id)) => {
                match self.get_node(*parent_id) {
                    Some(StructInfo::Item(_)) => match &item.content.as_ref() {
                        Content::Type(ty) => {
                            ty.write().unwrap().store = Arc::downgrade(&store_ref);
                            item.parent.replace(Parent::Type(Arc::downgrade(ty)));
                        }
                        Content::WeakType(ty) => {
                            ty.upgrade().unwrap().write().unwrap().store =
                                Arc::downgrade(&store_ref);
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
                if let Some(left) = item.left.as_ref().and_then(|i| i.as_item()) {
                    item.parent = left.parent.clone();
                    item.parent_sub = left.parent_sub.clone();
                } else if let Some(right) = item.right.as_ref().and_then(|i| i.as_item()) {
                    item.parent = right.parent.clone();
                    item.parent_sub = right.parent_sub.clone();
                }
            }
            _ => {}
        };

        Ok(())
    }

    pub(crate) fn integrate(
        &mut self,
        struct_info: StructInfo,
        offset: u64,
        parent: Option<&mut YType>,
    ) -> JwstCodecResult {
        let mut struct_info = if let StructInfo::WeakItem(item) = struct_info {
            StructInfo::Item(item.upgrade().unwrap())
        } else {
            struct_info
        };

        match &mut struct_info {
            StructInfo::Item(item) => {
                // SAFETY:
                // before we integrate struct into store,
                // the struct => Arc<Item> is owned reference actually,
                // no one else refer to such item yet, we can safely mutable refer to it now.
                let this = unsafe { Item::inner_mut(item) };

                if offset > 0 {
                    this.id.clock += offset;
                    let left =
                        self.split_at_and_get_left(Id::new(this.id.client, this.id.clock - 1))?;
                    this.origin_left_id = Some(left.last_id());
                    this.left = Some(left.as_weak());
                    this.content = Arc::new(this.content.split(offset)?.1);
                }

                if let Some(Parent::Type(ty)) = &this.parent {
                    let ty = ty.upgrade().unwrap();
                    let mut parent_lock: Option<RwLockWriteGuard<YType>> = None;
                    let parent = if let Some(p) = parent {
                        p
                    } else {
                        let lock = ty.write().unwrap();
                        parent_lock = Some(lock);
                        parent_lock.as_deref_mut().unwrap()
                    };

                    let right_is_null_or_has_left = match &this.right {
                        None => true,
                        Some(StructInfo::Item(right)) => right.left.is_some(),
                        Some(StructInfo::WeakItem(right)) => {
                            right.upgrade().unwrap().left.is_some()
                        }
                        _ => false,
                    };
                    let left_has_other_right_than_self = match &this.left {
                        Some(StructInfo::Item(left)) => left.right != this.right,
                        Some(StructInfo::WeakItem(left)) => {
                            left.upgrade().unwrap().right != this.right
                        }
                        _ => false,
                    };

                    // conflicts
                    if (this.left.is_none() && right_is_null_or_has_left)
                        || left_has_other_right_than_self
                    {
                        // set the first conflicting item
                        let mut pending_conflict = if let Some(StructInfo::Item(left)) = &this.left
                        {
                            left.right.clone()
                        } else if let Some(parent_sub) = &this.parent_sub {
                            parent
                                .map
                                .as_ref()
                                .and_then(|m| m.get(parent_sub).cloned())
                                .map(|s| s.head())
                        } else {
                            parent.start.clone()
                        };
                        let mut conflicting_items = HashSet::new();
                        let mut items_before_origin = HashSet::new();

                        while let Some(conflict) = &pending_conflict {
                            if Some(conflict) == this.right.as_ref() {
                                break;
                            }
                            let conflict_id = conflict.id();

                            items_before_origin.insert(conflict_id);
                            conflicting_items.insert(conflict_id);

                            if let StructInfo::Item(conflict_item) = conflict {
                                if this.origin_left_id == conflict_item.origin_left_id {
                                    // case 1
                                    if conflict_id.client < this.id.client {
                                        this.left = Some(conflict.as_weak());
                                        conflicting_items.clear();
                                    } else if this.origin_right_id == conflict_item.origin_right_id
                                    {
                                        // `this` and `c` are conflicting and point to the same
                                        // integration points. The id decides which item comes first.
                                        // Since `this` is to the left of `c`, we can break here.
                                        break;
                                    }
                                } else if let Some(conflict_item_left) =
                                    conflict_item.origin_left_id
                                {
                                    if items_before_origin.contains(&conflict_item_left)
                                        && !conflicting_items.contains(&conflict_item_left)
                                    {
                                        this.left = Some(conflict.as_weak());
                                        conflicting_items.clear();
                                    }
                                } else {
                                    break;
                                }

                                pending_conflict = conflict_item.right.clone();
                            } else {
                                break;
                            }
                        }
                    }

                    // reconnect left/right
                    // has left, connect left <-> self <-> left.right
                    if let Some(left) = this.left.as_ref().and_then(|i| i.as_item()) {
                        // Safety:
                        // we are in the process of integrating a new item.
                        // we got the write lock of store, parent.
                        // it's safe to modify the internal item.
                        let left = unsafe { Item::inner_mut(&left) };
                        this.right = left.right.replace(struct_info.as_weak());
                    } else {
                        // no left, parent.start = this
                        this.right = if let Some(parent_sub) = &this.parent_sub {
                            parent
                                .map
                                .as_ref()
                                .and_then(|map| map.get(parent_sub))
                                .map(|n| n.head())
                        } else {
                            parent.start.replace(struct_info.as_weak())
                        }
                        .as_ref()
                        .map(|i| i.as_weak());
                    }

                    // has right, connect
                    if let Some(right) = &this.right.as_ref().and_then(|i| i.as_item()) {
                        // Safety:
                        // we are in the process of integrating a new item.
                        // we got the write lock of store, parent.
                        // it's safe to modify the internal item.
                        let right = unsafe { Item::inner_mut(right) };
                        right.left = Some(struct_info.as_weak());
                    } else {
                        // no right, parent.start = this, delete this.left
                        if let Some(parent_sub) = &this.parent_sub {
                            parent
                                .map
                                .get_or_insert(HashMap::default())
                                .insert(parent_sub.to_string(), struct_info.clone());

                            if let Some(left) = &this.left {
                                self.delete(left, Some(parent));
                            }
                        }
                    }

                    let parent_deleted = parent.item().map(|item| item.deleted()).unwrap_or(false);

                    // should delete
                    if parent_deleted || this.parent_sub.is_some() && this.origin_right_id.is_some()
                    {
                        self.delete_item(this, Some(parent));
                    } else {
                        // adjust parent length
                        if this.parent_sub.is_none() {
                            parent.len += this.len();
                        }
                    }

                    parent_lock.take();
                }
            }
            StructInfo::WeakItem(_) => unreachable!(),
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

    pub(crate) fn delete_item(&self, item: &Item, parent: Option<&mut YType>) {
        if item.deleted() {
            return;
        }

        let id = item.id;
        item.delete();
        let mut delete_set = self.delete_set.write().unwrap();
        delete_set.add(id.client, id.clock, item.len());

        if item.parent_sub.is_none() && item.countable() {
            if let Some(parent) = parent {
                parent.len -= item.len();
            } else if let Some(Parent::Type(ty)) = &item.parent {
                ty.upgrade().unwrap().write().unwrap().len -= item.len();
            }
        }

        match item.content.as_ref() {
            Content::Type(_) => {
                // TODO: remove all type items
            }
            Content::Doc { .. } => {
                // TODO: remove subdoc
            }
            _ => {}
        }
    }

    pub(crate) fn delete(&self, struct_info: &StructInfo, parent: Option<&mut YType>) {
        if let Some(item) = struct_info.as_item() {
            self.delete_item(item.as_ref(), parent);
        }
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
                        DocStore::split_node_at(items, idx, start - id.clock)?;
                        idx += 1;
                    }
                };

                while idx < items.len() {
                    let struct_info = items[idx].clone();
                    let id = struct_info.id();

                    if !struct_info.deleted() && id.clock < end {
                        // need to split the item
                        // -----item-----
                        //           ^end
                        if end < id.clock + struct_info.len() {
                            DocStore::split_node_at(items, idx, end - id.clock)?;
                        }
                        self.delete(&struct_info, None);
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
                        vec_struct_info.push_back(first_block.clone().split_at(offset)?.1);
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
        loom_model!({
            let doc_store = DocStore::new();
            let state = doc_store.get_state(1);
            assert_eq!(state, 0);
        });

        loom_model!({
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
        });
    }

    #[test]
    fn test_get_state_vector() {
        loom_model!({
            let doc_store = DocStore::new();
            let state_map = doc_store.get_state_vector();
            assert!(state_map.is_empty());
        });

        loom_model!({
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
        });
    }

    #[test]
    fn test_add_item() {
        loom_model!({
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
        });
    }

    #[test]
    fn test_get_item() {
        loom_model!({
            let doc_store = DocStore::new();
            let struct_info = StructInfo::GC {
                id: Id::new(1, 0),
                len: 10,
            };
            doc_store.add_item(struct_info.clone()).unwrap();

            assert_eq!(doc_store.get_node(Id::new(1, 9)), Some(struct_info));
        });

        loom_model!({
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

            assert_eq!(doc_store.get_node(Id::new(1, 25)), Some(struct_info2));
        });

        loom_model!({
            let doc_store = DocStore::new();

            assert_eq!(doc_store.get_node(Id::new(1, 0)), None);
        });

        loom_model!({
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

            assert_eq!(doc_store.get_node(Id::new(1, 35)), None);
        });
    }

    #[test]
    fn test_split_and_get() {
        loom_model!({
            let doc_store = DocStore::new();
            let struct_info1 = StructInfo::Item(Arc::new(
                ItemBuilder::new()
                    .id((1, 0).into())
                    .content(Content::String(String::from("octo")))
                    .build(),
            ));

            let struct_info2 = StructInfo::Item(Arc::new(
                ItemBuilder::new()
                    .id((1, struct_info1.len()).into())
                    .left_id(Some(struct_info1.id()))
                    .content(Content::String(String::from("base")))
                    .build(),
            ));
            doc_store.add_item(struct_info1.clone()).unwrap();
            doc_store.add_item(struct_info2).unwrap();

            let s1 = doc_store.get_node(Id::new(1, 0)).unwrap();
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
        });
    }
}
