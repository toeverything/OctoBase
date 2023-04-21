use super::*;
use std::collections::{hash_map::Entry, HashMap};

pub struct DocStore {
    items: HashMap<u64, Vec<StructInfo>>,
}

impl DocStore {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn addItem(&mut self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client_id();

        match self.items.entry(client_id) {
            Entry::Occupied(mut entry) => {
                let structs = entry.get_mut();
                let last_struct = structs.last_mut().unwrap();
                let expect = last_struct.clock() + last_struct.len();
                let actually = item.clock();
                if expect != actually {
                    return Err(JwstCodecError::InvalidStructClock { expect, actually });
                }
                structs.push(item);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![item]);
            }
        }

        Ok(())
    }
}
