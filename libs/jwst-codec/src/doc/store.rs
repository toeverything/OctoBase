use super::*;
use crate::doc::StateVector;
use std::collections::VecDeque;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{hash_map::Entry, HashMap, HashSet},
    ops::{Deref, DerefMut, Range},
    sync::{Arc, RwLock},
};

#[derive(Clone, PartialEq, Debug)]
pub struct StructRef(Arc<RefCell<StructInfo>>);

impl StructRef {
    pub fn read<R: CrdtReader>(decoder: &mut R, id: Id) -> JwstCodecResult<Self> {
        match StructInfo::read(decoder, id) {
            Ok(info) => Ok(info.into()),
            Err(err) => Err(err),
        }
    }
}

impl<W: CrdtWriter> CrdtWrite<W> for StructRef {
    fn write(&self, writer: &mut W) -> JwstCodecResult {
        self.0.borrow().write(writer)
    }
}

impl From<StructInfo> for StructRef {
    fn from(info: StructInfo) -> Self {
        Self(Arc::new(RefCell::new(info)))
    }
}

impl From<Arc<RefCell<StructInfo>>> for StructRef {
    fn from(info: Arc<RefCell<StructInfo>>) -> Self {
        Self(info)
    }
}

impl Deref for StructRef {
    type Target = Arc<RefCell<StructInfo>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StructRef {
    pub fn as_ref(&self) -> Ref<'_, StructInfo> {
        self.0.borrow()
    }

    pub fn as_mut_ref(&self) -> RefMut<'_, StructInfo> {
        self.0.borrow_mut()
    }

    pub fn modify<M>(&self, mut modifier: M)
    where
        M: FnMut(&mut StructInfo),
    {
        let mut borrow = self.0.borrow_mut();
        modifier(&mut borrow);
    }

    pub fn id(&self) -> Id {
        self.0.borrow().id()
    }

    pub fn client(&self) -> Client {
        self.0.borrow().client()
    }

    pub fn clock(&self) -> Clock {
        self.0.borrow().clock()
    }

    pub fn len(&self) -> u64 {
        self.0.borrow().len()
    }

    pub fn is_item(&self) -> bool {
        self.0.borrow().is_item()
    }

    pub fn is_gc(&self) -> bool {
        self.0.borrow().is_gc()
    }

    pub fn is_skip(&self) -> bool {
        self.0.borrow().is_skip()
    }

    pub fn left_id(&self) -> Option<Id> {
        self.0.borrow().left_id()
    }

    pub fn right_id(&self) -> Option<Id> {
        self.0.borrow().right_id()
    }

    pub fn deleted(&self) -> bool {
        self.0.borrow().flags().deleted()
    }

    pub fn split_item(&mut self, diff: u64) -> JwstCodecResult<Self> {
        self.0.borrow_mut().split_item(diff).map(|info| info.into())
    }
}

#[derive(Default)]
pub struct DocStore {
    client: Client,
    pub items: Arc<RwLock<HashMap<Client, Vec<StructRef>>>>,
    pub delete_set: Arc<RwLock<DeleteSet>>,

    // following fields are only used in memory
    pub types: Arc<RwLock<HashMap<String, TypeStoreRef>>>,
    pub pending: Option<Update>,
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
                let last_struct = last_struct.as_ref();
                state.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        state
    }

    #[allow(dead_code)]
    pub fn add_item(&self, item: StructInfo) -> JwstCodecResult {
        self.add_item_ref(item.into())
    }

    pub fn add_item_ref(&self, item: StructRef) -> JwstCodecResult {
        let client_id = item.client();
        match self.items.write().unwrap().entry(client_id) {
            Entry::Occupied(mut entry) => {
                let structs = entry.get_mut();
                if let Some(last_struct) = structs.last() {
                    let last_struct = last_struct.as_ref();
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
    pub fn get_item_index(items: &Vec<StructRef>, clock: Clock) -> Option<usize> {
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

    pub fn get_item<I: Into<Id>>(&self, id: I) -> JwstCodecResult<StructRef> {
        self.get_item_with_idx(id).map(|(item, _)| item)
    }

    pub fn get_item_with_idx<I: Into<Id>>(&self, id: I) -> JwstCodecResult<(StructRef, usize)> {
        let id = id.into();
        if let Some(items) = self.items.read().unwrap().get(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                return Ok((items.get(index).unwrap().clone(), index));
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    #[allow(dead_code)]
    pub fn split_item<I: Into<Id>>(
        &mut self,
        id: I,
        diff: u64,
    ) -> JwstCodecResult<(StructRef, StructRef)> {
        debug_assert!(diff > 0);

        let id = id.into();

        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(idx) = Self::get_item_index(items, id.clock) {
                let item = items.get(idx).unwrap().clone();
                if item.is_item() {
                    let right_item: StructRef = item.borrow_mut().split_item(diff)?.into();
                    items.insert(idx + 1, right_item.clone());
                    return Ok((item, right_item));
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn get_item_clean_start<I: Into<Id>>(&self, id: I) -> JwstCodecResult<StructRef> {
        let id = id.into();
        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                let item = items.get(index).unwrap().clone();
                let offset = id.clock - item.clock();
                if offset > 0 && item.is_item() {
                    let right_item = item.borrow_mut().split_item(offset)?;
                    items.insert(index + 1, right_item.into());

                    return Ok((items.get(index + 1).unwrap()).clone());
                } else {
                    return Ok(item);
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    pub fn get_item_clean_end<I: Into<Id>>(&self, id: I) -> JwstCodecResult<StructRef> {
        let id = id.into();
        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                let item = items.get(index).unwrap().clone();
                let offset = id.clock - item.clock();
                if offset != item.len() - 1 && !item.is_gc() {
                    let right_item = item.borrow_mut().split_item(offset + 1)?;
                    items.insert(index + 1, right_item.into());

                    return Ok(items.get(index).unwrap().clone());
                } else {
                    return Ok(item);
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn replace_item(&self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client();
        let clock = item.clock();

        if let Some(structs) = self.items.write().unwrap().get_mut(&client_id) {
            let mut left = 0;
            let mut right = structs.len() - 1;
            let middle = &structs[right];
            let middle_clock = middle.clock();
            if middle_clock == clock {
                structs[middle_clock as usize] = item.into();
                return Ok(());
            }
            let mut middle_index = (clock / (middle_clock + middle.len() - 1)) as usize * right;
            while left <= right {
                let middle = &structs[middle_index];
                let middle_clock = middle.clock();
                if middle_clock <= clock {
                    if clock < middle_clock + middle.len() {
                        structs[middle_index] = item.into();
                        return Ok(());
                    }
                    left = middle_index + 1;
                } else {
                    right = middle_index - 1;
                }
                middle_index = (left + right) / 2;
            }
            Err(JwstCodecError::StructSequenceInvalid { client_id, clock })
        } else {
            Err(JwstCodecError::StructSequenceNotExists(client_id))
        }
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

    pub(crate) fn get_or_create_type(&self, str: &str) -> TypeStoreRef {
        match self.types.write().unwrap().entry(str.to_string()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let type_store = Arc::new(RefCell::new(TypeStore::new(TypeStoreKind::Unknown)));
                e.insert(type_store.clone());

                type_store
            }
        }
    }

    pub(crate) fn get_start_item(&self, s: StructRef) -> StructRef {
        let mut o = s;

        while let Some(left_id) = o.left_id() {
            if let Ok(left_struct) = self.get_item(left_id) {
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

    pub(crate) fn repair(&mut self, info: &mut StructInfo) -> JwstCodecResult {
        if let StructInfo::Item { item, .. } = info {
            if let Some(left_id) = item.left_id {
                let left = self.get_item_clean_end(left_id)?;
                item.left_id = Some(left.id());
            }

            if let Some(right_id) = item.right_id {
                let right = self.get_item_clean_start(right_id)?;
                item.right_id = Some(right.id());
            }

            match &item.parent {
                // root level named type
                Some(Parent::String(str)) => {
                    self.get_or_create_type(str);
                }
                // type as item
                Some(Parent::Id(parent_id)) => {
                    let parent = self.get_item(*parent_id)?;
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
                        let left = self.get_item(left_id)?;
                        item.parent = left.borrow().parent().cloned();
                        item.parent_sub = left.borrow().parent_sub().cloned();
                    } else if let Some(right_id) = item.right_id {
                        let right = self.get_item(right_id)?;
                        item.parent = right.borrow().parent().cloned();
                        item.parent_sub = right.borrow().parent_sub().cloned();
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn integrate_struct_info(
        &mut self,
        mut struct_info: StructInfo,
        offset: u64,
    ) -> JwstCodecResult {
        self.repair(&mut struct_info)?;
        let struct_info = Arc::new(RefCell::new(struct_info));
        match struct_info.borrow_mut().deref_mut() {
            StructInfo::Item { id, item } => {
                let mut left = item
                    .left_id
                    .and_then(|left_id| match self.get_item(left_id) {
                        Ok(left) => Some(left),
                        _ => None,
                    });

                let mut right = item
                    .right_id
                    .and_then(|right_id| match self.get_item(right_id) {
                        Ok(right) => Some(right),
                        _ => None,
                    });

                let parent = if let Some(parent) = &item.parent {
                    match parent {
                        Parent::String(name) => Some(self.get_or_create_type(name)),
                        Parent::Id(id) => {
                            if let Some(item) = self.get_item(*id)?.borrow().item() {
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
                    left = Some(self.get_item_clean_end(Id::new(id.client, id.clock - 1))?);
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
                            left.right_id()
                                .and_then(|right_id| match self.get_item(right_id) {
                                    Ok(item) => Some(item),
                                    Err(_) => None,
                                })
                        } else if let Some(parent_sub) = &item.parent_sub {
                            let o = parent.map.get(parent_sub).cloned();
                            o.as_ref().map(|o| self.get_start_item(o.clone()))
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
                                        Some(right_id) => Some(self.get_item(right_id)?),
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
                                    parent
                                        .map
                                        .get(parent_sub)
                                        .cloned()
                                        .map(|r| self.get_start_item(r))
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
                                    self.delete(left.id(), left.len());
                                    left.as_mut_ref().delete();
                                }
                            }
                        }

                        // should delete
                        if parent.item.is_some() && parent.item.clone().unwrap().deleted()
                            || item.parent_sub.is_some() && item.right_id.is_some()
                        {
                            self.delete(*id, item.len());
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
        self.add_item_ref(struct_info.into())
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
                            self.delete(id, struct_info.len());
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
                        vec_struct_info.push_back(
                            first_block.clone().borrow_mut().split_item(offset)?.clone(),
                        );
                    } else {
                        vec_struct_info.push_back(first_block.borrow().clone());
                    }

                    for item in items.iter().skip(index + 1) {
                        vec_struct_info.push_back(item.borrow().clone());
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

            doc_store.items.write().unwrap().insert(
                client_id,
                vec![struct_info1.into(), struct_info2.clone().into()],
            );

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
                .insert(client1, vec![struct_info1.clone().into()]);
            doc_store.items.write().unwrap().insert(
                client2,
                vec![struct_info2.into(), struct_info3.clone().into()],
            );

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

            assert_eq!(doc_store.get_item(Id::new(1, 9)), Ok(struct_info.into()));
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

            assert_eq!(doc_store.get_item(Id::new(1, 25)), Ok(struct_info2.into()));
        }

        {
            let doc_store = DocStore::new();

            assert_eq!(
                doc_store.get_item(Id::new(1, 0)),
                Err(JwstCodecError::StructSequenceNotExists(1))
            );
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

            assert_eq!(
                doc_store.get_item(Id::new(1, 35)),
                Err(JwstCodecError::StructSequenceNotExists(1))
            );
        }
    }

    #[test]
    fn test_get_item_clean() {
        let doc_store = DocStore::new();
        let struct_info1 = StructInfo::Item {
            id: Id::new(1, 0),
            item: Box::new(Item {
                content: Content::String(String::from("octo")),
                ..Default::default()
            }),
        };

        let struct_info2 = StructInfo::Item {
            id: Id::new(1, struct_info1.len()),
            item: Box::new(Item {
                left_id: Some(struct_info1.id()),
                content: Content::String(String::from("base")),
                ..Default::default()
            }),
        };
        doc_store.add_item(struct_info1.clone()).unwrap();
        doc_store.add_item(struct_info2).unwrap();

        let s1 = doc_store.get_item(Id::new(1, 0)).unwrap();
        assert_eq!(s1, struct_info1.into());
        let left = doc_store.get_item_clean_end((1, 1)).unwrap();
        assert_eq!(left.len(), 2); // octo => oc_to

        // s1 used to be (1, 4), but it actually ref of first item in store, so now it should be (1, 2)
        assert_eq!(
            s1, left,
            "doc internal mutation should not modify the pointer"
        );
        let right = doc_store.get_item_clean_start((1, 5)).unwrap();
        assert_eq!(right.len(), 3); // base => b_ase
    }
}
