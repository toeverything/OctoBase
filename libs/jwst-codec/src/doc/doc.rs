use std::collections::HashMap;

use super::*;

pub struct Doc {
    client_id: u64,
    guid: String,
    share: HashMap<String, Item>,
}

impl Doc {
    pub fn new() -> Self {
        Self {
            client_id: rand::random(),
            guid: nanoid!(),
            share: HashMap::new(),
        }
    }
}
