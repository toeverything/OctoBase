use std::collections::HashMap;

use super::*;

pub struct Doc {
    client_id: u64,
    guid: String,
    // root_type: HashMap<String, Item>,
    store: DocStore,
}

impl Doc {
    pub fn new() -> Self {
        Self {
            client_id: rand::random(),
            guid: nanoid!(),
            // share: HashMap::new(),
            store: DocStore::new(),
        }
    }
}
