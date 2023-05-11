use std::collections::{HashMap, HashSet};

use super::*;

struct RestItems {
    missing_state_vector: HashMap<u64, u64>,
    // TODO: use function in code
    #[allow(dead_code)]
    items: HashMap<u64, Vec<StructInfo>>,
    stage_items: Vec<StructInfo>,
}

impl RestItems {
    fn new() -> Self {
        Self {
            missing_state_vector: HashMap::new(),
            items: HashMap::new(),
            stage_items: Vec::new(),
        }
    }

    fn update_missing_sv(&mut self, client: u64, clock: u64) {
        self.missing_state_vector
            .entry(client)
            .and_modify(|mclock| {
                if *mclock > clock {
                    *mclock = clock;
                }
            })
            .or_insert(clock);
    }

    fn add_item(&mut self, item: StructInfo) {
        self.stage_items.push(item);
    }

    // TODO: use function in code
    #[allow(dead_code)]
    fn collect_items(&mut self) -> HashSet<u64> {
        let mut client_ids = HashSet::new();
        for item in self.stage_items.drain(..) {
            let client_id = item.client_id();
            // TODO: move all items of the current iterator to rest_items
            client_ids.insert(client_id);
        }
        client_ids
    }
}

pub struct Doc {
    // TODO: use function in code
    #[allow(dead_code)]
    // random client id for each doc
    client_id: u64,
    // TODO: use function in code
    #[allow(dead_code)]
    // random id for each doc, use in sub doc
    guid: String,
    // root_type: HashMap<String, Item>,
    store: DocStore,
}

impl Default for Doc {
    fn default() -> Self {
        Self {
            client_id: rand::random(),
            guid: nanoid!(),
            // share: HashMap::new(),
            store: DocStore::new(),
        }
    }
}

impl Doc {
    fn integrate_update(
        &self,
        items: &HashMap<u64, Vec<StructInfo>>,
    ) -> JwstCodecResult<Option<RestItems>> {
        let mut client_ids = items.keys().copied().collect::<Vec<_>>();
        if client_ids.is_empty() {
            return Ok(None);
        }
        client_ids.sort();
        client_ids.reverse();

        let mut rest_items = RestItems::new();
        let mut state_cache = HashMap::new();

        for client_id in client_ids {
            let refs = items.get(&client_id).unwrap();
            let mut iterator = refs.iter();
            while let Some(item) = iterator.next() {
                debug_assert_eq!(item.client_id(), client_id);
                if !item.is_skip() {
                    let local_clock = *state_cache
                        .entry(client_id)
                        .or_insert_with(|| self.store.get_state(client_id));
                    let offset = local_clock as i64 - item.clock() as i64;

                    if offset < 0 {
                        // applying update depends on another update that not exists on local
                        rest_items.add_item(item.clone());
                        rest_items.update_missing_sv(client_id, item.clock() - 1);
                        // TODO: move all items of the current iterator to rest_items
                        // rest_items.collect_items();
                    } else {
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn apply_update(&self, update: &[u8]) -> JwstCodecResult {
        let (rest, update) = read_update(update).map_err(|e| e.map_input(|u| u.len()))?;
        if rest.is_empty() {
            return Err(JwstCodecError::UpdateNotFullyConsumed(rest.len()));
        }
        self.integrate_update(&update.structs)?;

        Ok(())
    }
}
