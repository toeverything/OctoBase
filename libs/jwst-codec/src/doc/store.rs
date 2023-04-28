use super::*;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct DocStore {
    items: Arc<RwLock<HashMap<u64, Vec<StructInfo>>>>,
}

impl DocStore {
    pub fn new() -> Self {
        Self {
            items: Arc::default(),
        }
    }

    pub fn get_state(&self, client: u64) -> u64 {
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

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn get_state_vector(&self) -> HashMap<u64, u64> {
        let mut sm = HashMap::new();
        for (client, structs) in self.items.read().unwrap().iter() {
            if let Some(last_struct) = structs.last() {
                sm.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        sm
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
                structs.push(item);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![item]);
            }
        }

        Ok(())
    }

    // TODO: use function in code
    #[allow(dead_code)]
    // binary search struct info on a sorted array
    pub fn get_item_index(
        items: &Vec<StructInfo>,
        client_id: u64,
        clock: u64,
    ) -> JwstCodecResult<usize> {
        let mut left = 0;
        let mut right = items.len() - 1;
        let middle = &items[right];
        let middle_clock = middle.clock();
        if middle_clock == clock {
            return Ok(right);
        }
        let mut middle_index = (clock / (middle_clock + middle.len() - 1)) as usize * right;
        while left <= right {
            let middle = &items[middle_index];
            let middle_clock = middle.clock();
            if middle_clock <= clock {
                if clock < middle_clock + middle.len() {
                    return Ok(middle_index);
                }
                left = middle_index + 1;
            } else {
                right = middle_index - 1;
            }
            middle_index = (left + right) / 2;
        }
        Err(JwstCodecError::StructSequenceInvalid { client_id, clock })
    }

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn get_item(&self, client_id: u64, clock: u64) -> JwstCodecResult<StructInfo> {
        if let Some(items) = self.items.read().unwrap().get(&client_id) {
            let index = Self::get_item_index(items, client_id, clock)?;
            // TODO: item need to be a reference
            Ok(items[index].clone())
        } else {
            Err(JwstCodecError::StructSequenceNotExists(client_id))
        }
    }

    // TODO: use function in code
    #[allow(dead_code)]
    pub fn get_item_clean_end(&self, id: Id) -> JwstCodecResult<StructInfo> {
        if let Some(items) = self.items.read().unwrap().get(&id.client) {
            let index = Self::get_item_index(items, id.client, id.client)?;
            let mut item = items[index].clone();
            if id.clock != item.clock() + item.len() - 1 && !item.is_gc() {
                let (left_item, right_item) = item.split_item(id.clock - item.clock() + 1)?;
                if let Some(items) = self.items.write().unwrap().get_mut(&id.client) {
                    if let Some(index) = items.iter().position(|i| i.clock() == id.clock) {
                        items[index] = left_item.clone();
                        items.insert(index + 1, right_item);
                        // TODO: item need to be a reference
                        return Ok(left_item);
                    }
                }
                Err(JwstCodecError::ItemSplitNotSupport)
            } else {
                Ok(item)
            }
        } else {
            Err(JwstCodecError::StructSequenceNotExists(id.client))
        }
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
                structs[middle_clock as usize] = item;
                return Ok(());
            }
            let mut middle_index = (clock / (middle_clock + middle.len() - 1)) as usize * right;
            while left <= right {
                let middle = &structs[middle_index];
                let middle_clock = middle.clock();
                if middle_clock <= clock {
                    if clock < middle_clock + middle.len() {
                        structs[middle_index] = item;
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

            doc_store
                .items
                .write()
                .unwrap()
                .insert(client_id, vec![struct_info1.clone(), struct_info2.clone()]);

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
                id: Id::new(1, 0),
                len: 5,
            };

            let client2 = 2;
            let struct_info2 = StructInfo::GC {
                id: Id::new(2, 0),
                len: 6,
            };
            let struct_info3 = StructInfo::Skip {
                id: Id::new(2, 6),
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
                .insert(client2, vec![struct_info2.clone(), struct_info3.clone()]);

            let state_map = doc_store.get_state_vector();

            assert_eq!(
                state_map.get(&client1),
                Some(&(struct_info1.clock() + struct_info1.len()))
            );
            assert_eq!(
                state_map.get(&client2),
                Some(&(struct_info3.clock() + struct_info3.len()))
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

            assert_eq!(doc_store.get_item(1, 9), Ok(struct_info));
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

            assert_eq!(doc_store.get_item(1, 25), Ok(struct_info2));
        }

        {
            let doc_store = DocStore::new();

            assert_eq!(
                doc_store.get_item(1, 0),
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
                doc_store.get_item(1, 35),
                Err(JwstCodecError::StructSequenceInvalid {
                    client_id: 1,
                    clock: 35
                })
            );
        }
    }
}
