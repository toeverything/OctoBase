use super::*;
use crate::doc::StateVector;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{hash_map::Entry, HashMap},
    ops::Deref,
    sync::{Arc, RwLock},
};

#[derive(Clone, PartialEq, Debug)]
pub struct StructRef(Arc<RefCell<StructInfo>>);

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
}

#[derive(Default)]
pub struct DocStore {
    pub items: Arc<RwLock<HashMap<Client, Vec<StructRef>>>>,
    pub types: Arc<RwLock<HashMap<String, TypeStoreRef>>>,
    pub delete_set: Arc<RwLock<DeleteSet>>,
    pub pending: Option<Update>,
}

impl DocStore {
    pub fn new() -> Self {
        Default::default()
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

    pub fn with_client_items_mut(
        &self,
        client: Client,
        mut modifier: impl FnMut(&mut Vec<StructRef>) -> JwstCodecResult,
    ) -> JwstCodecResult {
        if let Some(structs) = self.items.write().unwrap().get_mut(&client) {
            return modifier(structs);
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn split_item<I: Into<Id>>(&mut self, id: I, diff: u64) -> JwstCodecResult {
        if diff == 0 {
            return Ok(());
        }

        let id = id.into();

        if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
            if let Some(idx) = Self::get_item_index(items, id.clock) {
                let item = items.get(idx).unwrap().clone();
                if item.is_item() {
                    let right_item = item.borrow_mut().split_item(diff)?;
                    items.insert(idx + 1, right_item.into());
                }
            }
        }

        Ok(())
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

    pub(crate) fn delete(&self, id: Id, len: u64) {
        if let Ok(mut delete_set) = self.delete_set.write() {
            delete_set.add(id.client, id.clock, len)
        }
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
