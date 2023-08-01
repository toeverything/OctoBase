use super::*;
use crate::{
    doc::StateVector,
    sync::{Arc, RwLock, RwLockWriteGuard, Weak},
};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    mem,
    ops::Range,
};

#[derive(Default, Debug)]
pub(crate) struct DocStore {
    client: Client,
    pub items: HashMap<Client, Vec<Node>>,
    pub delete_set: DeleteSet,

    // following fields are only used in memory
    pub types: HashMap<String, YTypeRef>,
    // types created from this store but with no names,
    // we store it here to keep the ownership inside store without being released.
    pub dangling_types: HashMap<usize, YTypeRef>,
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
        if let Some(structs) = self.items.get(&client) {
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
        for (client, structs) in self.items.iter() {
            if let Some(last_struct) = structs.last() {
                state.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        state
    }

    pub fn add_item(&mut self, item: Node) -> JwstCodecResult {
        let client_id = item.client();
        match self.items.entry(client_id) {
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
    pub fn get_item_index(items: &Vec<Node>, clock: Clock) -> Option<usize> {
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

    pub fn create_item(
        &self,
        content: Content,
        left: Somr<Item>,
        right: Somr<Item>,
        parent: Option<Parent>,
        parent_sub: Option<String>,
    ) -> ItemRef {
        let id = (self.client(), self.get_state(self.client())).into();
        let item = Somr::new(Item::new(id, content, left, right, parent, parent_sub));

        if let Content::Type(ty) = item.get().unwrap().content.as_ref() {
            ty.get().unwrap().write().unwrap().item = item.clone();
        }

        item
    }

    pub fn get_node<I: Into<Id>>(&self, id: I) -> Option<Node> {
        self.get_node_with_idx(id).map(|(item, _)| item)
    }

    pub fn get_node_with_idx<I: Into<Id>>(&self, id: I) -> Option<(Node, usize)> {
        let id = id.into();
        if let Some(items) = self.items.get(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                return items.get(index).map(|item| (item.clone(), index));
            }
        }

        None
    }

    pub fn split_node<I: Into<Id>>(&mut self, id: I, diff: u64) -> JwstCodecResult<(Node, Node)> {
        debug_assert!(diff > 0);

        let id = id.into();

        if let Some(items) = self.items.get_mut(&id.client) {
            if let Some(idx) = Self::get_item_index(items, id.clock) {
                return Self::split_node_at(items, idx, diff);
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn split_node_at(
        items: &mut Vec<Node>,
        idx: usize,
        diff: u64,
    ) -> JwstCodecResult<(Node, Node)> {
        debug_assert!(diff > 0);

        let node = items.get(idx).unwrap().clone();
        debug_assert!(node.is_item());
        let (left_node, right_node) = node.split_at(diff)?;
        match (&node, &left_node, &right_node) {
            (Node::Item(raw_left_item), Node::Item(new_left_item), Node::Item(right_item)) => {
                // SAFETY:
                // we make sure store is the only entry of mutating an item,
                // and we already hold mutable reference of store, so it's safe to do so
                unsafe {
                    let left_item = raw_left_item.get_mut_unchecked();
                    let right_item = right_item.get_mut_unchecked();
                    left_item.content = new_left_item.get_unchecked().content.clone();

                    // we had the correct left/right content
                    // now build the references
                    let right_right_ref = ItemRef::from(&left_item.right);
                    right_item.left = if right_right_ref.is_some() {
                        let right_right = right_right_ref.get_mut_unchecked();
                        right_right.left.replace(right_node.clone())
                    } else {
                        Some(node.clone())
                    };
                    right_item.right = left_item.right.replace(right_node.clone());
                    right_item.origin_left_id = Some(left_item.last_id());
                    right_item.origin_right_id = left_item.origin_right_id;
                };
            }
            _ => {
                items[idx] = left_node;
            }
        }

        let right_ref = right_node.clone();
        items.insert(idx + 1, right_node);

        Ok((node, right_ref))
    }

    pub fn split_at_and_get_right<I: Into<Id>>(&mut self, id: I) -> JwstCodecResult<Node> {
        let id = id.into();
        if let Some(items) = self.items.get_mut(&id.client) {
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

    pub fn split_at_and_get_left<I: Into<Id>>(&mut self, id: I) -> JwstCodecResult<Node> {
        let id = id.into();
        if let Some(items) = self.items.get_mut(&id.client) {
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
        for structs in self.items.values() {
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
    pub fn get_or_create_type(&mut self, name: &str) -> YTypeRef {
        match self.types.entry(name.to_string()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let ty = Somr::new(RwLock::new(YType {
                    kind: YTypeKind::Unknown,
                    root_name: Some(name.to_string()),
                    ..Default::default()
                }));

                let ty_ref = ty.clone();
                e.insert(ty);
                ty_ref
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
    pub fn repair(&mut self, item: &mut Item, store_ref: StoreRef) -> JwstCodecResult {
        if let Some(left_id) = item.origin_left_id {
            item.left = Some(self.split_at_and_get_left(left_id)?);
        }

        if let Some(right_id) = item.origin_right_id {
            item.right = Some(self.split_at_and_get_right(right_id)?);
        }

        match &item.parent {
            // root level named type
            // doc.get_or_create_text("content");
            //                         ^^^^^^^ Parent::String("content")
            Some(Parent::String(str)) => {
                let ty = self.get_or_create_type(str);
                if let Some(ty) = ty.get() {
                    ty.write().unwrap().store = Arc::downgrade(&store_ref);
                }
                item.parent.replace(Parent::Type(ty));
            }
            // type as item
            // let text = doc.create_text();
            // let mut map = doc.get_or_create_map("content");
            // map.insert("p1", text);
            //
            // Item { id: (1, 0), content: Content::Type(_) }
            //        ^^ Parent::Id((1, 0))
            Some(Parent::Id(parent_id)) => {
                match self.get_node(*parent_id) {
                    Some(Node::Item(parent_item)) => {
                        match &parent_item.get().unwrap().content.as_ref() {
                            Content::Type(ty) => {
                                if let Some(ty) = ty.get() {
                                    ty.write().unwrap().store = Arc::downgrade(&store_ref);
                                }
                                item.parent.replace(Parent::Type(ty.clone()));
                            }
                            Content::Deleted(_) => {
                                // parent got deleted, take it.
                                item.parent.take();
                            }
                            _ => {
                                return Err(JwstCodecError::InvalidParent);
                            }
                        }
                    }
                    _ => {
                        // GC & Skip are not valid parent, take it.
                        item.parent.take();
                    }
                }
            }
            // no item.parent, borrow left.parent or right.parent
            None => {
                if let Some(left) = ItemRef::from(item.left.clone()).get() {
                    item.parent = left.parent.clone();
                    item.parent_sub = left.parent_sub.clone();
                } else if let Some(right) = ItemRef::from(item.right.clone()).get() {
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
        mut struct_info: Node,
        offset: u64,
        parent: Option<&mut YType>,
    ) -> JwstCodecResult {
        match &mut struct_info {
            Node::Item(item) => {
                assert!(
                    item.is_owned(),
                    "Required a owned Item type but got an shared reference"
                );

                // SAFETY:
                // before we integrate struct into store,
                // the struct => Arc<Item> is owned reference actually,
                // no one else refer to such item yet, we can safely mutable refer to it now.
                let this = unsafe { item.get_mut_unchecked() };

                if offset > 0 {
                    this.id.clock += offset;
                    let left =
                        self.split_at_and_get_left(Id::new(this.id.client, this.id.clock - 1))?;
                    this.origin_left_id = Some(left.last_id());
                    this.left = Some(left);
                    this.content = Arc::new(this.content.split(offset)?.1);
                }

                if let Some(Parent::Type(ty)) = &this.parent {
                    let mut parent_lock: Option<RwLockWriteGuard<YType>> = None;
                    let parent = if let Some(p) = parent {
                        p
                    } else if let Some(ty) = ty.get() {
                        let lock = ty.write().unwrap();
                        parent_lock = Some(lock);
                        parent_lock.as_deref_mut().unwrap()
                    } else {
                        return Ok(());
                    };

                    let mut left: ItemRef = this.left.as_ref().into();
                    let mut right: ItemRef = this.right.as_ref().into();

                    let right_is_null_or_has_left = match right.get() {
                        None => true,
                        Some(r) => ItemRef::from(&r.left).is_some(),
                    };

                    let left_has_other_right_than_self = match left.get() {
                        Some(left) => ItemRef::from(&left.right) != right,
                        _ => false,
                    };

                    // conflicts
                    if (left.is_none() && right_is_null_or_has_left)
                        || left_has_other_right_than_self
                    {
                        // set the first conflicting item
                        let mut conflict = if let Some(left) = left.get() {
                            left.right.as_ref().into()
                        } else if let Some(parent_sub) = &this.parent_sub {
                            parent
                                .map
                                .as_ref()
                                .and_then(|m| m.get(parent_sub).cloned())
                                .map(|s| s.as_item())
                                .unwrap_or(Somr::none())
                        } else {
                            parent.start.clone()
                        };

                        let mut conflicting_items = HashSet::new();
                        let mut items_before_origin = HashSet::new();

                        while conflict.is_some() {
                            if conflict == right {
                                break;
                            }
                            if let Some(conflict_item) = conflict.get() {
                                let conflict_id = conflict_item.id;

                                items_before_origin.insert(conflict_id);
                                conflicting_items.insert(conflict_id);

                                if this.origin_left_id == conflict_item.origin_left_id {
                                    // case 1
                                    if conflict_id.client < this.id.client {
                                        left = conflict.clone();
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
                                        left = conflict.clone();
                                        conflicting_items.clear();
                                    }
                                } else {
                                    break;
                                }

                                conflict = ItemRef::from(&conflict_item.right);
                            } else {
                                break;
                            }
                        }
                    }

                    // reconnect left/right
                    // has left, connect left <-> self <-> left.right
                    if left.is_some() {
                        unsafe {
                            // SAFETY: we get store write lock, no way the left get dropped by owner
                            let left = left.get_mut_unchecked();
                            right = left.right.replace(Node::Item(item.clone())).into();
                        }
                        this.left = Some(Node::Item(left));
                    } else {
                        // no left, parent.start = this
                        right = if let Some(parent_sub) = &this.parent_sub {
                            parent
                                .map
                                .as_ref()
                                .and_then(|map| map.get(parent_sub))
                                .map(|n| n.head())
                                .into()
                        } else {
                            mem::replace(&mut parent.start, item.clone())
                        };
                        this.left = None;
                    }

                    // has right, connect
                    if right.is_some() {
                        unsafe {
                            // SAFETY: we get store write lock, no way the left get dropped by owner
                            let right = right.get_mut_unchecked();
                            right.left = Some(Node::Item(item.clone()));
                        }
                        this.right = Some(Node::Item(right))
                    } else {
                        // no right, parent.start = this, delete this.left
                        if let Some(parent_sub) = &this.parent_sub {
                            parent
                                .map
                                .get_or_insert(HashMap::default())
                                .insert(parent_sub.to_string(), Node::Item(item.clone()));

                            if let Some(left) = &this.left {
                                self.delete(left, Some(parent));
                            }
                        }
                    }

                    let parent_deleted = parent
                        .item
                        .get()
                        .map(|item| item.deleted())
                        .unwrap_or(false);

                    // should delete
                    if parent_deleted || this.parent_sub.is_some() && this.origin_right_id.is_some()
                    {
                        Self::delete_item(this, Some(parent));
                    } else {
                        // adjust parent length
                        if this.parent_sub.is_none() {
                            parent.len += this.len();
                        }
                    }

                    parent_lock.take();
                }
            }
            Node::GC(item) => {
                if offset > 0 {
                    item.id.clock += offset;
                    item.len -= offset;
                }
            }
            Node::Skip(_) => {
                // skip ignored
            }
        }
        self.add_item(struct_info)
    }

    pub(crate) fn delete_item(item: &Item, parent: Option<&mut YType>) {
        if !item.delete() {
            return;
        }

        if item.parent_sub.is_none() && item.countable() {
            if let Some(parent) = parent {
                parent.len -= item.len();
            } else if let Some(Parent::Type(ty)) = &item.parent {
                ty.get().unwrap().write().unwrap().len -= item.len();
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

    pub(crate) fn delete(&self, struct_info: &Node, parent: Option<&mut YType>) {
        if let Some(item) = struct_info.as_item().get() {
            Self::delete_item(item, parent);
        }
    }

    pub fn delete_range(&mut self, client: u64, range: Range<u64>) -> JwstCodecResult {
        let start = range.start;
        let end = range.end;

        if let Some(items) = self.items.get_mut(&client) {
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

                    if let Some(item) = struct_info.as_item().get() {
                        if !item.deleted() && id.clock < end {
                            // need to split the item
                            // -----item-----
                            //           ^end
                            if end < id.clock + struct_info.len() {
                                DocStore::split_node_at(items, idx, end - id.clock)?;
                            }

                            self.delete_set.add(client, id.clock, item.len());
                            Self::delete_item(item, None);
                        } else {
                            break;
                        }
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
        let mut update_structs: HashMap<u64, VecDeque<Node>> = HashMap::new();

        for (client, clock) in diff {
            // We have made sure that the client is in the local state vector in diff_state_vectors()
            if let Some(items) = self.items.get(&client) {
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
            delete_set: self.delete_set.clone(),
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
            let doc_store = DocStore::with_client(1);
            let state = doc_store.get_state(1);
            assert_eq!(state, 0);
        });

        loom_model!({
            let mut doc_store = DocStore::with_client(1);

            let client_id = 1;

            let struct_info1 = Node::GC(NodeLen::new(Id::new(1, 1), 5));
            let struct_info2 = Node::Skip(NodeLen::new(Id::new(1, 6), 7));

            doc_store
                .items
                .insert(client_id, vec![struct_info1, struct_info2.clone()]);

            let state = doc_store.get_state(client_id);

            assert_eq!(state, struct_info2.clock() + struct_info2.len());

            assert!(doc_store.self_check().is_ok());
        });
    }

    #[test]
    fn test_get_state_vector() {
        loom_model!({
            let doc_store = DocStore::with_client(1);
            let state_map = doc_store.get_state_vector();
            assert!(state_map.is_empty());
        });

        loom_model!({
            let mut doc_store = DocStore::with_client(1);

            let client1 = 1;
            let struct_info1 = Node::GC(NodeLen::new((1, 0).into(), 5));

            let client2 = 2;
            let struct_info2 = Node::GC(NodeLen::new((2, 0).into(), 6));
            let struct_info3 = Node::Skip(NodeLen::new((2, 6).into(), 1));

            doc_store.items.insert(client1, vec![struct_info1.clone()]);
            doc_store
                .items
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
            let mut doc_store = DocStore::with_client(1);

            let struct_info1 = Node::GC(NodeLen::new(Id::new(1, 0), 5));
            let struct_info2 = Node::Skip(NodeLen::new(Id::new(1, 5), 1));
            let struct_info3_err = Node::Skip(NodeLen::new(Id::new(1, 5), 1));
            let struct_info3 = Node::Skip(NodeLen::new(Id::new(1, 6), 1));

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
            let mut doc_store = DocStore::with_client(1);
            let struct_info = Node::GC(NodeLen::new(Id::new(1, 0), 10));
            doc_store.add_item(struct_info.clone()).unwrap();

            assert_eq!(doc_store.get_node(Id::new(1, 9)), Some(struct_info));
        });

        loom_model!({
            let mut doc_store = DocStore::with_client(1);
            let struct_info1 = Node::GC(NodeLen::new(Id::new(1, 0), 10));
            let struct_info2 = Node::GC(NodeLen::new(Id::new(1, 10), 20));
            doc_store.add_item(struct_info1).unwrap();
            doc_store.add_item(struct_info2.clone()).unwrap();

            assert_eq!(doc_store.get_node(Id::new(1, 25)), Some(struct_info2));
        });

        loom_model!({
            let doc_store = DocStore::with_client(1);

            assert_eq!(doc_store.get_node(Id::new(1, 0)), None);
        });

        loom_model!({
            let mut doc_store = DocStore::with_client(1);
            let struct_info1 = Node::GC(NodeLen::new(Id::new(1, 0), 10));
            let struct_info2 = Node::GC(NodeLen::new(Id::new(1, 10), 20));
            doc_store.add_item(struct_info1).unwrap();
            doc_store.add_item(struct_info2).unwrap();

            assert_eq!(doc_store.get_node(Id::new(1, 35)), None);
        });
    }

    #[test]
    fn test_split_node_at() {
        loom_model!({
            let node = Node::Item(Somr::new(
                ItemBuilder::new()
                    .id((1, 0).into())
                    .content(Content::String(String::from("octo")))
                    .build(),
            ));
            let mut list = vec![node.clone()];

            let (left, right) = DocStore::split_node_at(&mut list, 0, 2).unwrap();

            assert_eq!(
                node.as_item().ptr().as_ptr() as usize,
                left.as_item().ptr().as_ptr() as usize
            );
            assert_eq!(node.len(), 2);
            assert_eq!(left.len(), 2);
            assert_eq!(right.len(), 2);
            assert_eq!(left.right(), Some(right.clone()));
            assert_eq!(right.left(), Some(left));
        });
    }

    #[test]
    fn test_split_and_get() {
        loom_model!({
            let mut doc_store = DocStore::with_client(1);
            let struct_info1 = Node::Item(Somr::new(
                ItemBuilder::new()
                    .id((1, 0).into())
                    .content(Content::String(String::from("octo")))
                    .build(),
            ));

            let struct_info2 = Node::Item(Somr::new(
                ItemBuilder::new()
                    .id((1, struct_info1.len()).into())
                    .left_id(Some(struct_info1.id()))
                    .content(Content::String(String::from("base")))
                    .build(),
            ));
            let s1_ref = struct_info1.clone();
            doc_store.add_item(struct_info1).unwrap();
            doc_store.add_item(struct_info2).unwrap();

            let s1 = doc_store.get_node(Id::new(1, 0)).unwrap();
            assert_eq!(s1, s1_ref);
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
