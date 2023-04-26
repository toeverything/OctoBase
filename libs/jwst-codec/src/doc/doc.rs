use std::collections::HashMap;

use super::*;

struct RestStructs {
    missing_state_vector: HashMap<u64, u64>,
    items: HashMap<u64, Vec<StructInfo>>,
}

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

    fn integrate_update(
        &self,
        items: &HashMap<u64, Vec<StructInfo>>,
    ) -> JwstCodecResult<Option<RestStructs>> {
        let mut client_ids = items.keys().copied().collect::<Vec<_>>();
        if client_ids.len() == 0 {
            return Ok(None);
        }
        client_ids.sort();

        let mut refs = items.get(client_ids.last().unwrap()).unwrap();
        let mut rest_store = DocStore::new();

        let mut missing_sv = HashMap::new();
        let update_missing_sv = |client: u64, clock: u64| {
            missing_sv
                .entry(client)
                .and_modify(|mclock| {
                    if *mclock > clock {
                        *mclock = clock;
                    }
                })
                .or_insert(clock);
        };

        Ok(None)
    }

    pub fn apply_update(&self, update: &[u8]) -> JwstCodecResult {
        let (rest, update) = read_update(update).map_err(|e| e.map_input(|u| u.len()))?;
        if rest.len() > 0 {
            return Err(JwstCodecError::UpdateNotFullyConsumed(rest.len()));
        }
        self.integrate_update(&update.structs)?;

        Ok(())
    }
}
