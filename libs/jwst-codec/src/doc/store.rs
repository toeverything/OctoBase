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

    pub fn get_state(&self, client: u64) -> u64 {
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

    pub fn get_state_vector(&self) -> HashMap<u64, u64> {
        let mut sm = HashMap::new();
        for (client, structs) in self.items.iter() {
            if let Some(last_struct) = structs.last() {
                sm.insert(*client, last_struct.clock() + last_struct.len());
            } else {
                warn!("client {} has no struct info", client);
            }
        }
        sm
    }

    pub fn add_item(&mut self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client_id();

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

    // binary search struct info on a sorted array
    pub fn get_item(&self, client_id: u64, clock: u64) -> JwstCodecResult<&StructInfo> {
        if let Some(structs) = self.items.get(&client_id) {
            let mut left = 0;
            let mut right = structs.len() - 1;
            let middle = &structs[right];
            let middle_clock = middle.clock();
            if middle_clock == clock {
                return Ok(middle);
            }
            let mut middle_index = (clock / (middle_clock + middle.len() - 1)) as usize * right;
            while left <= right {
                let middle = &structs[middle_index];
                let middle_clock = middle.clock();
                if middle_clock <= clock {
                    if clock < middle_clock + middle.len() {
                        return Ok(middle);
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

    fn get_item_clean_end(&self, id: Id) -> JwstCodecResult<&StructInfo> {
        let item = self.get_item(id.client, id.client)?;
        if id.clock != item.clock() + item.len() - 1 && !item.is_gc() {
            // structs.splice(index + 1, 0, transaction.splitItem(struct, id.clock - struct.id.clock + 1))
        }
        Ok(item)
    }

    pub fn replace_struct(&mut self, item: StructInfo) -> JwstCodecResult {
        let client_id = item.client_id();
        let clock = item.clock();

        if let Some(structs) = self.items.get_mut(&client_id) {
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

    pub fn self_check(&self) -> JwstCodecResult {
        for structs in self.items.values() {
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
