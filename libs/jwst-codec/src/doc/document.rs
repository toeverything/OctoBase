use super::*;

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
        let client = rand::random();
        Self {
            client_id: client,
            guid: nanoid!(),
            // share: HashMap::new(),
            store: DocStore::with_client(client),
        }
    }
}

impl Doc {
    pub fn client(&self) -> Client {
        self.client_id
    }

    pub fn new_from_binary(binary: Vec<u8>) -> JwstCodecResult<Self> {
        let mut doc = Doc::default();
        doc.apply_update_from_binary(binary)?;
        Ok(doc)
    }

    pub fn apply_update_from_binary(&mut self, update: Vec<u8>) -> JwstCodecResult {
        let mut decoder = RawDecoder::new(update);
        let update = Update::read(&mut decoder)?;
        self.apply_update(update)?;
        Ok(())
    }

    pub fn apply_update(&mut self, mut update: Update) -> JwstCodecResult {
        let mut retry = false;
        loop {
            for (s, offset) in update.iter(self.store.get_state_vector()) {
                self.store.integrate_struct_info(s, offset)?;
            }

            for (client, range) in update.delete_set_iter(self.store.get_state_vector()) {
                self.store.delete_range(client, range)?;
            }

            if let Some(mut pending_update) = self.store.pending.take() {
                if pending_update
                    .missing_state
                    .iter()
                    .any(|(client, clock)| self.store.get_state(*client) > *clock)
                {
                    // new update has been applied to the doc, need to re-integrate
                    retry = true;
                }

                for (client, range) in pending_update.delete_set_iter(self.store.get_state_vector())
                {
                    self.store.delete_range(client, range)?;
                }

                if update.is_pending_empty() {
                    update = pending_update;
                } else {
                    // drain all pending state to pending update for later iteration
                    update.drain_pending_state();
                    update = Update::merge([pending_update, update]);
                }
            } else {
                // no pending update at store

                // no pending update in current iteration
                // thank god, all clean
                if update.is_pending_empty() {
                    break;
                } else {
                    // need to turn all pending state into update for later iteration
                    update.drain_pending_state();
                };
            }

            // can't integrate any more, save the pending update
            if !retry {
                if !update.is_pending_empty() {
                    self.store.pending = Some(update)
                }
                break;
            }
        }

        Ok(())
    }

    pub fn get_text(&mut self, name: &str) -> TypeStoreRef {
        let text = self.store.get_or_create_type(name);
        text.borrow_mut().set_kind(TypeStoreKind::Text);
        text
    }

    pub fn encode_update_v1(&self) -> JwstCodecResult<Vec<u8>> {
        self.encode_state_as_update_v1(&StateVector::default())
    }

    pub fn encode_state_as_update_v1(&self, sv: &StateVector) -> JwstCodecResult<Vec<u8>> {
        let store = &self.store;
        let mut encoder = RawEncoder::default();
        store.encode_with_state_vector(sv, &mut encoder)?;

        Ok(encoder.into_inner())
    }

    pub fn get_state_vector(&self) -> StateVector {
        self.store.get_state_vector()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Map, Transact};

    #[test]
    fn double_run_test_with_yrs_basic() {
        let yrs_doc = yrs::Doc::new();

        let map = yrs_doc.get_or_insert_map("abc");
        let mut trx = yrs_doc.transact_mut();
        map.insert(&mut trx, "a", 1).unwrap();

        let binary_from_yrs = trx.encode_update_v1().unwrap();
        let doc = Doc::new_from_binary(binary_from_yrs.clone()).unwrap();
        let binary = doc.encode_update_v1().unwrap();

        assert_eq!(binary_from_yrs, binary);
    }

    #[test]
    fn test_encode_state_as_update() {
        let yrs_doc = yrs::Doc::new();

        let map = yrs_doc.get_or_insert_map("abc");
        let mut trx = yrs_doc.transact_mut();
        map.insert(&mut trx, "a", 1).unwrap();
        let binary_from_yrs = trx.encode_update_v1().unwrap();

        let yrs_doc_new = yrs::Doc::new();
        let array = yrs_doc_new.get_or_insert_array("array");
        let mut trx = yrs_doc_new.transact_mut();
        array.insert(&mut trx, 0, "array_value").unwrap();
        let binary_from_yrs_new = trx.encode_update_v1().unwrap();

        let mut doc = Doc::new_from_binary(binary_from_yrs.clone()).unwrap();
        let mut doc_new = Doc::new_from_binary(binary_from_yrs_new.clone()).unwrap();

        let diff_update = doc_new
            .encode_state_as_update_v1(&doc.get_state_vector())
            .unwrap();

        let diff_update_reverse = doc
            .encode_state_as_update_v1(&doc_new.get_state_vector())
            .unwrap();

        doc.apply_update_from_binary(diff_update).unwrap();
        doc_new
            .apply_update_from_binary(diff_update_reverse)
            .unwrap();

        assert_eq!(
            doc.encode_update_v1().unwrap(),
            doc_new.encode_update_v1().unwrap()
        );
    }
}
