use super::*;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock},
};

type StructRef = Arc<StructInfo>;

#[derive(Clone)]
pub struct DocStore {
    items: Arc<RwLock<HashMap<u64, Vec<StructRef>>>>,
}

impl DocStore {
    pub fn new() -> Self {
        Self {
            items: Arc::default(),
        }
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
        let mut state = StateVector::new();
        for (client, structs) in self.items.read().unwrap().iter() {
            if let Some(last_struct) = structs.last() {
                state.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        state
    }

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn add_item(&self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client_id();

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
                structs.push(Arc::new(item));
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![Arc::new(item)]);
            }
        }

        Ok(())
    }

    // binary search struct info on a sorted array
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
        let id = id.into();
        if let Some(items) = self.items.read().unwrap().get(&id.client) {
            if let Some(index) = Self::get_item_index(items, id.clock) {
                return Ok(Arc::clone(items.get(index).unwrap()));
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
                    let (left_item, right_item) = item.split_item(offset)?;
                    items[index] = left_item.into();
                    items.insert(index + 1, right_item.into());

                    return Ok(Arc::clone(items.get(index + 1).unwrap()));
                } else {
                    return Ok(Arc::clone(&item));
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
                    let (left_item, right_item) = item.split_item(offset + 1)?;
                    items[index] = left_item.into();
                    items.insert(index + 1, right_item.into());

                    return Ok(Arc::clone(items.get(index).unwrap()));
                } else {
                    return Ok(Arc::clone(&item));
                }
            }
        }

        Err(JwstCodecError::StructSequenceNotExists(id.client))
    }

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn replace_item(&self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client_id();
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
                        client_id: l.client_id(),
                        clock: l.clock(),
                    });
                }
            }
        }

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
                vec![struct_info1.clone().into(), struct_info2.clone().into()],
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
                vec![struct_info2.clone().into(), struct_info3.clone().into()],
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
        assert!(doc_store.add_item(struct_info2.clone()).is_ok());
        assert_eq!(
            doc_store.add_item(struct_info3_err),
            Err(JwstCodecError::StructClockInvalid {
                expect: 6,
                actually: 5
            })
        );
        assert!(doc_store.add_item(struct_info3.clone()).is_ok());
        assert_eq!(
            doc_store.get_state(struct_info1.client_id()),
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
            doc_store.add_item(struct_info1.clone()).unwrap();
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
            doc_store.add_item(struct_info1.clone()).unwrap();
            doc_store.add_item(struct_info2.clone()).unwrap();

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
                left_id: None,
                right_id: None,
                parent: None,
                parent_sub: None,
                content: Content::String(String::from("octo")),
            }),
        };

        let struct_info2 = StructInfo::Item {
            id: Id::new(1, struct_info1.len()),
            item: Box::new(Item {
                left_id: Some(*struct_info1.id()),
                right_id: None,
                parent: None,
                parent_sub: None,
                content: Content::String(String::from("base")),
            }),
        };
        doc_store.add_item(struct_info1).unwrap();
        doc_store.add_item(struct_info2).unwrap();

        let left = doc_store.get_item_clean_end((1, 1)).unwrap();
        assert_eq!(left.len(), 2); // octo => oc_to
        let right = doc_store.get_item_clean_start((1, 5)).unwrap();
        assert_eq!(right.len(), 3); // base => b_ase
    }
}
