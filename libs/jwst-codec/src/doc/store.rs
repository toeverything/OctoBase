use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    mem,
    ops::{Deref, Range},
};

use super::*;
use crate::{
    doc::StateVector,
    sync::{Arc, RwLock, RwLockWriteGuard, Weak},
};

unsafe impl Send for DocStore {}
unsafe impl Sync for DocStore {}

#[derive(Default, Debug)]
pub(crate) struct DocStore {
    client: Client,
    pub items: HashMap<Client, VecDeque<Node>>,
    pub delete_set: DeleteSet,

    // following fields are only used in memory
    pub types: HashMap<String, YTypeRef>,
    // types created from this store but with no names,
    // we store it here to keep the ownership inside store without being released.
    pub dangling_types: HashMap<usize, YTypeRef>,
    pub pending: Option<Update>,
    pub last_optimized_state: StateVector,
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

    pub fn clients(&self) -> Vec<Client> {
        self.items.keys().cloned().collect()
    }

    #[cfg(feature = "debug")]
    pub fn total_nodes(&self) -> usize {
        self.items.values().map(|v| v.len()).sum()
    }

    #[cfg(feature = "debug")]
    pub fn total_delete_sets(&self) -> usize {
        self.delete_set
            .values()
            .map(|v| match v {
                OrderRange::Range(_) => 1,
                OrderRange::Fragment(f) => f.len(),
            })
            .sum()
    }

    #[cfg(feature = "debug")]
    pub fn total_types(&self) -> usize {
        self.types.len()
    }

    #[cfg(feature = "debug")]
    pub fn total_dangling_types(&self) -> usize {
        self.dangling_types.len()
    }

    #[cfg(feature = "debug")]
    pub fn total_pending_nodes(&self) -> usize {
        self.pending.as_ref().map(|p| p.structs.len()).unwrap_or(0)
    }

    pub fn get_state(&self, client: Client) -> Clock {
        if let Some(structs) = self.items.get(&client) {
            if let Some(last_struct) = structs.back() {
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
        Self::items_as_state_vector(&self.items)
    }

    fn items_as_state_vector(items: &HashMap<Client, VecDeque<Node>>) -> StateVector {
        let mut state = StateVector::default();
        for (client, structs) in items.iter() {
            if let Some(last_struct) = structs.back() {
                state.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        state
    }

    pub fn add_node(&mut self, item: Node) -> JwstCodecResult {
        let client_id = item.client();
        match self.items.entry(client_id) {
            Entry::Occupied(mut entry) => {
                let structs = entry.get_mut();
                if let Some(last_struct) = structs.back() {
                    let expect = last_struct.clock() + last_struct.len();
                    let actually = item.clock();
                    if expect != actually {
                        return Err(JwstCodecError::StructClockInvalid { expect, actually });
                    }
                } else {
                    warn!("client {} has no struct info", client_id);
                }
                structs.push_back(item);
            }
            Entry::Vacant(entry) => {
                entry.insert(VecDeque::from([item]));
            }
        }

        Ok(())
    }

    /// binary search struct info on a sorted array
    pub fn get_node_index(items: &VecDeque<Node>, clock: Clock) -> Option<usize> {
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
            if let Some(mut ty) = ty.ty_mut() {
                ty.item = item.clone();
            }
        }

        item
    }

    pub fn get_node<I: Into<Id>>(&self, id: I) -> Option<Node> {
        self.get_node_with_idx(id).map(|(item, _)| item)
    }

    pub fn get_node_with_idx<I: Into<Id>>(&self, id: I) -> Option<(Node, usize)> {
        let id = id.into();
        if let Some(items) = self.items.get(&id.client) {
            if let Some(index) = Self::get_node_index(items, id.clock) {
                return items.get(index).map(|item| (item.clone(), index));
            }
        }

        None
    }

    pub fn split_node<I: Into<Id>>(&mut self, id: I, diff: u64) -> JwstCodecResult<(Node, Node)> {
        debug_assert!(diff > 0);

        let id = id.into();

        if let Some(items) = self.items.get_mut(&id.client) {
            if let Some(idx) = Self::get_node_index(items, id.clock) {
                return Self::split_node_at(items, idx, diff);
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn split_node_at(items: &mut VecDeque<Node>, idx: usize, diff: u64) -> JwstCodecResult<(Node, Node)> {
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
                    let mut left_item = raw_left_item.get_mut_unchecked();
                    let mut right_item = right_item.get_mut_unchecked();
                    left_item.content = new_left_item.get_unchecked().content.clone();

                    // we had the correct left/right content
                    // now build the references
                    let right_right_ref = ItemRef::from(&left_item.right);
                    right_item.left = if right_right_ref.is_some() {
                        let mut right_right = right_right_ref.get_mut_unchecked();
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
            if let Some(index) = Self::get_node_index(items, id.clock) {
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
            if let Some(index) = Self::get_node_index(items, id.clock) {
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
    pub fn get_or_create_type(&mut self, store_ref: &StoreRef, name: &str) -> YTypeRef {
        match self.types.entry(name.to_string()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let mut inner = YType::new(YTypeKind::Unknown, None);
                inner.root_name = Some(name.to_string());

                let ty = YTypeRef {
                    store: Arc::downgrade(store_ref),
                    inner: Somr::new(RwLock::new(inner)),
                };
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
    ///     - [Parent::String] for root level named type (e.g
    ///       `doc.get_or_create_text("content")`)
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
                let ty = self.get_or_create_type(&store_ref, str);
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
                                item.parent.replace(Parent::Type(ty.clone()));
                            }
                            _ => {
                                // invalid parent, take it.
                                item.parent.take();
                                // return Err(JwstCodecError::InvalidParent);
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

        // assign store in ytype to ensure store exists if a ytype not has any children
        if let Content::Type(ty) = Arc::make_mut(&mut item.content) {
            ty.store = Arc::downgrade(&store_ref);

            // we keep ty owner in dangling_types so the delete of any type will not make it
            // dropped
            if ty.inner.is_owned() {
                let owned_inner = ty.inner.swap_take();
                self.dangling_types.insert(
                    ty.inner.ptr().as_ptr() as usize,
                    YTypeRef {
                        store: ty.store.clone(),
                        inner: owned_inner,
                    },
                );
            } else {
                return Err(JwstCodecError::InvalidParent);
            }
        }

        Ok(())
    }

    pub fn integrate(&mut self, mut node: Node, offset: u64, parent: Option<&mut YType>) -> JwstCodecResult {
        match &mut node {
            Node::Item(item) => {
                assert!(
                    item.is_owned(),
                    "Required a owned Item type but got an shared reference"
                );

                // SAFETY:
                // before we integrate struct into store,
                // the struct => Arc<Item> is owned reference actually,
                // no one else refer to such item yet, we can safely mutable refer to it now.
                let this = &mut *unsafe { item.get_mut_unchecked() };

                if offset > 0 {
                    this.id.clock += offset;
                    let left = self.split_at_and_get_left(Id::new(this.id.client, this.id.clock - 1))?;
                    this.origin_left_id = Some(left.last_id());
                    this.left = Some(left);
                    this.content = Arc::new(this.content.split(offset)?.1);
                }

                if let Some(Parent::Type(ty)) = &this.parent {
                    let mut parent_lock: Option<RwLockWriteGuard<YType>> = None;
                    let parent = if let Some(p) = parent {
                        p
                    } else if let Some(ty) = ty.ty_mut() {
                        parent_lock = Some(ty);
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
                    if (left.is_none() && right_is_null_or_has_left) || left_has_other_right_than_self {
                        // set the first conflicting item
                        let mut conflict = if let Some(left) = left.get() {
                            left.right.as_ref().into()
                        } else if let Some(parent_sub) = &this.parent_sub {
                            parent.map.get(parent_sub).cloned().unwrap_or(Somr::none())
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
                                    } else if this.origin_right_id == conflict_item.origin_right_id {
                                        // `this` and `c` are conflicting and point to the same
                                        // integration points. The id decides which item comes first.
                                        // Since `this` is to the left of `c`, we can break here.
                                        break;
                                    }
                                } else if let Some(conflict_item_left) = conflict_item.origin_left_id {
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
                            let mut left = left.get_mut_unchecked();
                            right = left.right.replace(Node::Item(item.clone())).into();
                        }
                        this.left = Some(Node::Item(left));
                    } else {
                        // no left, parent.start = this
                        right = if let Some(parent_sub) = &this.parent_sub {
                            parent.map.get(parent_sub).map(|n| Node::Item(n.clone()).head()).into()
                        } else {
                            mem::replace(&mut parent.start, item.clone())
                        };
                        this.left = None;
                    }

                    // has right, connect
                    if right.is_some() {
                        unsafe {
                            // SAFETY: we get store write lock, no way the left get dropped by owner
                            let mut right = right.get_mut_unchecked();
                            right.left = Some(Node::Item(item.clone()));
                        }
                        this.right = Some(Node::Item(right))
                    } else {
                        // no right, parent.start = this, delete this.left
                        if let Some(parent_sub) = &this.parent_sub {
                            parent.map.insert(parent_sub.to_string(), item.clone());

                            if let Some(left) = &this.left {
                                self.delete_node(left, Some(parent));
                            }
                        }
                    }

                    let parent_deleted = parent.item.get().map(|item| item.deleted()).unwrap_or(false);

                    // should delete
                    if parent_deleted || this.parent_sub.is_some() && this.right.is_some() {
                        self.delete_node(&Node::Item(item.clone()), Some(parent));
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
        self.add_node(node)
    }

    pub fn delete_item(&mut self, item: &Item, parent: Option<&mut YType>) {
        Self::delete_item_inner(&mut self.delete_set, item, parent);
    }

    fn delete_item_inner(delete_set: &mut DeleteSet, item: &Item, parent: Option<&mut YType>) {
        if !item.delete() {
            return;
        }

        delete_set.add(item.id.client, item.id.clock, item.len());

        if item.parent_sub.is_none() && item.countable() {
            if let Some(parent) = parent {
                if parent.len != 0 {
                    parent.len -= item.len();
                }
            } else if let Some(Parent::Type(ty)) = &item.parent {
                ty.ty_mut().unwrap().len -= item.len();
            }
        }

        match item.content.as_ref() {
            Content::Type(ty) => {
                if let Some(mut ty) = ty.ty_mut() {
                    // items in ty are all refs, not owned
                    let mut item_ref = ty.start.clone();
                    while let Some(item) = item_ref.get() {
                        // delete_set
                        Self::delete_item_inner(delete_set, item, Some(&mut ty));

                        if let Some(right) = &item.right {
                            item_ref = right.as_item();
                        } else {
                            item_ref = Somr::none();
                        }
                    }

                    let map_values = ty.map.values().cloned().collect::<Vec<_>>();
                    for item in map_values {
                        if let Some(item) = item.get() {
                            Self::delete_item_inner(delete_set, item, Some(&mut ty));
                        }
                    }
                }
            }
            Content::Doc { .. } => {
                // TODO: remove subdoc
            }
            _ => {}
        }
    }

    pub fn delete_node(&mut self, struct_info: &Node, parent: Option<&mut YType>) {
        if let Some(item) = struct_info.as_item().get() {
            self.delete_item(item, parent);
        }
    }

    pub fn delete_range(&mut self, client: u64, range: Range<u64>) -> JwstCodecResult {
        let start = range.start;
        let end = range.end;

        if let Some(items) = self.items.get_mut(&client) {
            if let Some(mut idx) = DocStore::get_node_index(items, start) {
                {
                    // id.clock <= range.start < id.end
                    // need to split the item and delete the right part
                    // -----item-----
                    //    ^start
                    let node = &items[idx];
                    let id = node.id();

                    if !node.deleted() && id.clock < start {
                        DocStore::split_node_at(items, idx, start - id.clock)?;
                        idx += 1;
                    }
                };

                while idx < items.len() {
                    let node = items[idx].clone();
                    let id = node.id();

                    if id.clock < end {
                        if !node.deleted() {
                            if let Some(item) = node.as_item().get() {
                                // need to split the item
                                // -----item-----
                                //           ^end
                                if end < id.clock + node.len() {
                                    DocStore::split_node_at(items, idx, end - id.clock)?;
                                }

                                Self::delete_item_inner(&mut self.delete_set, item, None);
                            }
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

    fn diff_state_vectors(local_state_vector: &StateVector, remote_state_vector: &StateVector) -> Vec<(Client, Clock)> {
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

    pub fn diff_state_vector(&self, sv: &StateVector) -> JwstCodecResult<Update> {
        let update_structs = Self::diff_structs(&self.items, sv)?;

        let mut update = Update {
            structs: update_structs,
            delete_set: Self::generate_delete_set(&self.items),
            ..Update::default()
        };

        if let Some(pending) = &self.pending {
            Update::merge_into(&mut update, [pending.clone()])
        }

        Ok(update)
    }

    fn diff_structs(
        map: &HashMap<Client, VecDeque<Node>>,
        sv: &StateVector,
    ) -> JwstCodecResult<HashMap<Client, VecDeque<Node>>> {
        let local_state_vector = Self::items_as_state_vector(map);
        let diff = Self::diff_state_vectors(&local_state_vector, sv);
        let mut update_structs: HashMap<u64, VecDeque<Node>> = HashMap::new();

        for (client, clock) in diff {
            // We have made sure that the client is in the local state vector in
            // diff_state_vectors()
            if let Some(items) = map.get(&client) {
                if items.is_empty() {
                    continue;
                }

                update_structs.insert(client, VecDeque::new());
                let vec_struct_info = update_structs.get_mut(&client).unwrap();

                // the smallest clock in items may exceed the clock
                let clock = items.front().unwrap().id().clock.max(clock);
                if let Some(index) = Self::get_node_index(items, clock) {
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

        Ok(update_structs)
    }

    fn generate_delete_set(refs: &HashMap<Client, VecDeque<Node>>) -> DeleteSet {
        let mut delete_set = DeleteSet::default();

        for (client, nodes) in refs {
            for node in nodes {
                if node.deleted() {
                    delete_set.add(*client, node.id().clock, node.len());
                }
            }
        }

        delete_set
    }

    /// Optimize the memory usage of store
    pub fn optimize(&mut self) -> JwstCodecResult {
        //  1. gc delete set
        self.gc_delete_set()?;
        //  2. merge delete set (in our delete set impl, which is based on `OrderRange`
        //     has already have auto-merge functionality) pass
        //  3. merge same content siblings, e.g contentString + ContentString
        self.make_continuous()
    }

    fn gc_delete_set(&mut self) -> JwstCodecResult<()> {
        for (client, deletes) in self.delete_set.deref() {
            for range in deletes {
                let start = range.start;
                let end = range.end;
                let items = self.items.get_mut(client).unwrap();
                if let Some(mut idx) = Self::get_node_index(items, start) {
                    while idx < items.len() {
                        if let Node::Item(item) = items[idx].clone() {
                            let item = unsafe { item.get_unchecked() };

                            if item.id.clock >= end {
                                break;
                            }

                            if !item.keep() {
                                Self::gc_item(items, idx, false)?;
                            }
                        }

                        idx += 1;
                    }
                }
            }
        }

        Ok(())
    }

    fn gc_item_by_id(items: &mut VecDeque<Node>, id: Id, replace: bool) -> JwstCodecResult {
        if let Some(idx) = Self::get_node_index(items, id.clock) {
            Self::gc_item(items, idx, replace)?;
        }

        Ok(())
    }

    fn gc_item(items: &mut VecDeque<Node>, idx: usize, replace: bool) -> JwstCodecResult {
        if let Node::Item(item_ref) = items[idx].clone() {
            let item = unsafe { item_ref.get_unchecked() };

            if !item.deleted() {
                return Err(JwstCodecError::Unexpected);
            }

            Self::gc_content(items, &item.content)?;

            if replace {
                let _ = mem::replace(&mut items[idx], Node::new_gc(item.id, item.len()));
            } else {
                let mut item = unsafe { item_ref.get_mut_unchecked() };
                item.content = Arc::new(Content::Deleted(item.len()));
                item.flags.clear_countable();
                debug_assert!(!item.flags.countable());
            }
        }

        Ok(())
    }

    fn gc_content(items: &mut VecDeque<Node>, content: &Content) -> JwstCodecResult {
        if let Content::Type(ty) = content {
            if let Some(mut ty) = ty.ty_mut() {
                // items in ty are all refs, not owned
                let mut item_ref = ty.start.clone();
                while let Some(item) = item_ref.get() {
                    let id = item.id;

                    // we need to iter to right first, because we may delete the owned item
                    // by replacing it with [Node::GC]
                    if let Some(right) = &item.right {
                        item_ref = right.as_item();
                    } else {
                        item_ref = Somr::none();
                    }

                    Self::gc_item_by_id(items, id, true)?;
                }
                ty.start = Somr::none();

                for item in ty.map.values() {
                    if let Some(item) = item.get() {
                        Self::gc_item_by_id(items, item.id, true)?;
                    }
                }

                ty.map.clear();
            }
        }

        Ok(())
    }

    fn make_continuous(&mut self) -> JwstCodecResult {
        let state = self.get_state_vector();

        for (client, state) in state.iter() {
            let before_state = self.last_optimized_state.get(client);
            if before_state == *state {
                continue;
            }

            let nodes = self.items.get_mut(client).unwrap();
            let first_change = Self::get_node_index(nodes, before_state).unwrap_or(1).max(1);
            let mut idx = nodes.len() - 1;

            while idx > 0 && idx >= first_change {
                idx = idx.saturating_sub(Self::merge_with_lefts(nodes, idx) + 1);
            }
        }

        self.last_optimized_state = state;
        Ok(())
    }

    fn merge_with_lefts(nodes: &mut VecDeque<Node>, idx: usize) -> usize {
        let mut pos = idx;
        loop {
            if pos == 0 {
                break;
            }

            let right = nodes.get(pos).unwrap().clone();
            let left = nodes.get_mut(pos - 1).unwrap();

            match (left, right) {
                (Node::GC(left), Node::GC(right)) => {
                    left.len += right.len;
                }
                (Node::Skip(left), Node::Skip(right)) => {
                    left.len += right.len;
                }
                (Node::Item(lref), Node::Item(rref)) => {
                    let mut litem = unsafe { lref.get_mut_unchecked() };
                    let mut ritem = unsafe { rref.get_mut_unchecked() };
                    let llen = litem.len();

                    if litem.id.client != ritem.id.client
                        // not same delete status
                        || litem.deleted() != ritem.deleted()
                        // not clock continuous
                        || litem.id.clock + litem.len() != ritem.id.clock
                        // not insertion continuous
                        || Some(litem.last_id()) != ritem.origin_left_id
                        // not insertion continuous
                        || litem.origin_right_id != ritem.origin_right_id
                        // not runtime continuous
                        || litem.right_item() != rref
                    {
                        break;
                    }

                    match (Arc::make_mut(&mut litem.content), Arc::make_mut(&mut ritem.content)) {
                        (Content::Deleted(l), Content::Deleted(r)) => {
                            *l += *r;
                        }
                        (Content::Json(l), Content::Json(r)) => {
                            l.extend(r.drain(0..));
                        }
                        (Content::String(l), Content::String(r)) => {
                            *l += r;
                        }
                        (Content::Any(l), Content::Any(r)) => {
                            l.extend(r.drain(0..));
                        }
                        _ => {
                            break;
                        }
                    }

                    if let Some(Parent::Type(p)) = &litem.parent {
                        if let Some(parent) = p.ty_mut() {
                            if let Some(markers) = &parent.markers {
                                markers.replace_marker(rref.clone(), lref.clone(), -(llen as i64));
                            }
                        }
                    }

                    if ritem.keep() {
                        litem.flags.set_keep()
                    }

                    litem.right = ritem.right.clone();

                    if let Some(Node::Item(right)) = &litem.right {
                        let mut right = unsafe { right.get_mut_unchecked() };
                        right.left = Some(Node::Item(lref.clone()))
                    }
                }
                _ => {
                    break;
                }
            }

            pos -= 1;
        }
        nodes.drain(pos + 1..=idx);

        // return the index of processed items
        idx - pos
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

            let struct_info1 = Node::new_gc(Id::new(1, 1), 5);
            let struct_info2 = Node::new_skip(Id::new(1, 6), 7);

            doc_store
                .items
                .insert(client_id, VecDeque::from([struct_info1, struct_info2.clone()]));

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
            let struct_info1 = Node::new_gc((1, 0).into(), 5);

            let client2 = 2;
            let struct_info2 = Node::new_gc((2, 0).into(), 6);
            let struct_info3 = Node::new_skip((2, 6).into(), 1);

            doc_store.items.insert(client1, VecDeque::from([struct_info1.clone()]));
            doc_store
                .items
                .insert(client2, VecDeque::from([struct_info2, struct_info3.clone()]));

            let state_map = doc_store.get_state_vector();

            assert_eq!(state_map.get(&client1), struct_info1.clock() + struct_info1.len());
            assert_eq!(state_map.get(&client2), struct_info3.clock() + struct_info3.len());

            assert!(doc_store.self_check().is_ok());
        });
    }

    #[test]
    fn test_add_item() {
        loom_model!({
            let mut doc_store = DocStore::with_client(1);

            let struct_info1 = Node::new_gc(Id::new(1, 0), 5);
            let struct_info2 = Node::new_skip(Id::new(1, 5), 1);
            let struct_info3_err = Node::new_skip(Id::new(1, 5), 1);
            let struct_info3 = Node::new_skip(Id::new(1, 6), 1);

            assert!(doc_store.add_node(struct_info1.clone()).is_ok());
            assert!(doc_store.add_node(struct_info2).is_ok());
            assert_eq!(
                doc_store.add_node(struct_info3_err),
                Err(JwstCodecError::StructClockInvalid { expect: 6, actually: 5 })
            );
            assert!(doc_store.add_node(struct_info3.clone()).is_ok());
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
            let struct_info = Node::new_gc(Id::new(1, 0), 10);
            doc_store.add_node(struct_info.clone()).unwrap();

            assert_eq!(doc_store.get_node(Id::new(1, 9)), Some(struct_info));
        });

        loom_model!({
            let mut doc_store = DocStore::with_client(1);
            let struct_info1 = Node::new_gc(Id::new(1, 0), 10);
            let struct_info2 = Node::new_gc(Id::new(1, 10), 20);
            doc_store.add_node(struct_info1).unwrap();
            doc_store.add_node(struct_info2.clone()).unwrap();

            assert_eq!(doc_store.get_node(Id::new(1, 25)), Some(struct_info2));
        });

        loom_model!({
            let doc_store = DocStore::with_client(1);

            assert_eq!(doc_store.get_node(Id::new(1, 0)), None);
        });

        loom_model!({
            let mut doc_store = DocStore::with_client(1);
            let struct_info1 = Node::new_gc(Id::new(1, 0), 10);
            let struct_info2 = Node::new_gc(Id::new(1, 10), 20);
            doc_store.add_node(struct_info1).unwrap();
            doc_store.add_node(struct_info2).unwrap();

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
            let mut list = VecDeque::from([node.clone()]);

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
            doc_store.add_node(struct_info1).unwrap();
            doc_store.add_node(struct_info2).unwrap();

            let s1 = doc_store.get_node(Id::new(1, 0)).unwrap();
            assert_eq!(s1, s1_ref);
            let left = doc_store.split_at_and_get_left((1, 1)).unwrap();
            assert_eq!(left.len(), 2); // octo => oc_to

            // s1 used to be (1, 4), but it actually ref of first item in store, so now it
            // should be (1, 2)
            assert_eq!(s1, left, "doc internal mutation should not modify the pointer");
            let right = doc_store.split_at_and_get_right((1, 5)).unwrap();
            assert_eq!(right.len(), 3); // base => b_ase
        });
    }

    #[test]
    fn should_replace_gc_item_with_content_deleted() {
        loom_model!({
            let item1 = ItemBuilder::new()
                .id((1, 0).into())
                .content(Content::String(String::from("octo")))
                .build();
            let item2 = ItemBuilder::new()
                .id((1, 4).into())
                .content(Content::String(String::from("base")))
                .build();

            item1.delete();

            let mut store = DocStore::with_client(1);

            store.add_node(Node::Item(Somr::new(item1))).unwrap();
            store.add_node(Node::Item(Somr::new(item2))).unwrap();
            store.delete_set.add_range(1, 0..4);

            store.gc_delete_set().unwrap();

            assert_eq!(
                store
                    .get_node((1, 0))
                    .unwrap()
                    .as_item()
                    .get()
                    .unwrap()
                    .content
                    .as_ref(),
                &Content::Deleted(4)
            );
        });
    }

    #[test]
    fn should_gc_type_items() {
        loom_model!({
            let doc = DocOptions::new().with_client_id(1).build();

            let mut arr = doc.get_or_create_array("arr").unwrap();
            let mut text = doc.create_text().unwrap();

            arr.insert(0, Value::from(text.clone())).unwrap();

            text.insert(0, "hello world").unwrap();
            text.remove(5, 6).unwrap();

            arr.remove(0, 1).unwrap();
            let mut store = doc.store.write().unwrap();
            store.gc_delete_set().unwrap();

            assert_eq!(arr.len(), 0);
            assert_eq!(
                store
                    .get_node((1, 0))
                    .unwrap()
                    .as_item()
                    .get()
                    .unwrap()
                    .content
                    .as_ref(),
                &Content::Deleted(1)
            );

            assert_eq!(
                store.get_node((1, 1)).unwrap(), // "hello" GCd
                Node::new_gc((1, 1).into(), 5)
            );

            assert_eq!(
                store.get_node((1, 7)).unwrap(), // " world" GCd
                Node::new_gc((1, 6).into(), 6)
            );
        });
    }

    #[test]
    fn should_merge_same_sibling_items() {
        loom_model!({
            let mut store = DocStore::with_client(1);
            store.items.insert(
                1,
                VecDeque::from([
                    Node::new_gc((1, 0).into(), 2),
                    Node::new_gc((1, 2).into(), 2),
                    Node::new_skip((1, 4).into(), 2),
                    Node::Item(Somr::new(
                        ItemBuilder::new()
                            .id((1, 6).into())
                            .content(Content::String(String::from("hello")))
                            .build(),
                    )),
                    // actually not mergable, due to runtime continuous check
                    // will cover it in [test_merge_same_sibling_items2]
                    Node::Item(Somr::new(
                        ItemBuilder::new()
                            .id((1, 11).into())
                            .content(Content::String(String::from("world")))
                            .left_id(Some((1, 11).into()))
                            .build(),
                    )),
                ]),
            );

            store.make_continuous().unwrap();

            assert_eq!(store.items.get(&1).unwrap().len(), 4);
        });
    }

    #[test]
    fn test_merge_same_sibling_items2() {
        loom_model!({
            let doc = Doc::new();

            let mut text = doc.get_or_create_text("text").unwrap();
            text.insert(0, "a").unwrap();
            text.insert(1, "b").unwrap();
            text.insert(2, "c").unwrap();
            text.insert(3, ", hello").unwrap();

            assert_eq!(text.to_string(), "abc, hello");

            let mut store = doc.store.write().unwrap();
            assert_eq!(store.items.get(&1).unwrap().len(), 4);
            store.make_continuous().unwrap();
            assert_eq!(store.items.get(&1).unwrap().len(), 1);
            assert_eq!(text.to_string(), "abc, hello");
        });
    }
}
