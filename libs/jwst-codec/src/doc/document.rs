use super::{publisher::DocPublisher, store::StoreRef, *};
use crate::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct DocOptions {
    pub guid: Option<String>,
    pub client: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Doc {
    client_id: u64,
    // random id for each doc, use in sub doc
    // TODO: use function in code
    #[allow(dead_code)]
    guid: String,
    pub(super) store: StoreRef,
    pub publisher: Arc<DocPublisher>,
}

unsafe impl Send for Doc {}
unsafe impl Sync for Doc {}

impl Default for Doc {
    fn default() -> Self {
        let client_id = rand::random();
        let store = Arc::new(RwLock::new(DocStore::with_client(client_id)));
        let publisher = Arc::new(DocPublisher::new(store.clone()));

        Self {
            client_id,
            guid: nanoid!(),
            store,
            publisher,
        }
    }
}

impl PartialEq for Doc {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
    }
}

impl Doc {
    pub fn with_options(options: DocOptions) -> Self {
        let client = options.client.unwrap_or_else(rand::random);
        let store = Arc::new(RwLock::new(DocStore::with_client(client)));
        let publisher = Arc::new(DocPublisher::new(store.clone()));

        Self {
            client_id: client,
            store,
            guid: options.guid.unwrap_or_else(|| nanoid!()),
            publisher,
        }
    }

    pub fn with_client(client_id: u64) -> Self {
        let store = Arc::new(RwLock::new(DocStore::with_client(client_id)));
        let publisher = Arc::new(DocPublisher::new(store.clone()));
        Self {
            client_id,
            store,
            guid: nanoid!(),
            publisher,
        }
    }

    pub fn client(&self) -> Client {
        self.client_id
    }

    pub fn guid(&self) -> &str {
        self.guid.as_str()
    }

    pub fn new_from_binary(binary: Vec<u8>) -> JwstCodecResult<Self> {
        let mut doc = Doc::default();
        doc.apply_update_from_binary(binary)?;
        Ok(doc)
    }

    pub fn new_from_binary_with_options(
        binary: Vec<u8>,
        options: DocOptions,
    ) -> JwstCodecResult<Self> {
        let mut doc = Doc::with_options(options);
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
        let mut store = self.store.write().unwrap();
        let mut retry = false;
        loop {
            for (mut s, offset) in update.iter(store.get_state_vector()) {
                if let Node::Item(item) = &mut s {
                    debug_assert!(item.is_owned());
                    let item = unsafe { item.get_mut_unchecked() };
                    store.repair(item, self.store.clone())?;
                }
                store.integrate(s, offset, None)?;
            }

            for (client, range) in update.delete_set_iter(store.get_state_vector()) {
                store.delete_range(client, range)?;
            }

            if let Some(mut pending_update) = store.pending.take() {
                if pending_update
                    .missing_state
                    .iter()
                    .any(|(client, clock)| store.get_state(*client) > *clock)
                {
                    // new update has been applied to the doc, need to re-integrate
                    retry = true;
                }

                for (client, range) in pending_update.delete_set_iter(store.get_state_vector()) {
                    store.delete_range(client, range)?;
                }

                if update.is_pending_empty() {
                    update = pending_update;
                } else {
                    // drain all pending state to pending update for later iteration
                    update.drain_pending_state();
                    Update::merge_into(&mut update, [&pending_update]);
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
                if !update.is_empty() {
                    store.pending.replace(update);
                }
                break;
            }
        }

        Ok(())
    }

    pub fn keys(&self) -> Vec<String> {
        let store = self.store.read().unwrap();
        store.types.keys().cloned().collect()
    }

    pub fn get_or_create_text(&self, name: &str) -> JwstCodecResult<Text> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Text)
            .set_name(name.to_string())
            .build()
    }

    pub fn create_text(&self) -> JwstCodecResult<Text> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Text)
            .build()
    }

    pub fn get_or_create_array(&self, str: &str) -> JwstCodecResult<Array> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Array)
            .set_name(str.to_string())
            .build()
    }

    pub fn create_array(&self) -> JwstCodecResult<Array> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Array)
            .build()
    }

    pub fn get_or_create_map(&self, str: &str) -> JwstCodecResult<Map> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Map)
            .set_name(str.to_string())
            .build()
    }

    pub fn create_map(&self) -> JwstCodecResult<Map> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Map)
            .build()
    }

    pub fn get_map(&self, str: &str) -> JwstCodecResult<Map> {
        YTypeBuilder::new(self.store.clone())
            .with_kind(YTypeKind::Map)
            .set_name(str.to_string())
            .build_exists()
    }

    pub fn encode_update_v1(&self) -> JwstCodecResult<Vec<u8>> {
        self.encode_state_as_update_v1(&StateVector::default())
    }

    pub fn encode_state_as_update_v1(&self, sv: &StateVector) -> JwstCodecResult<Vec<u8>> {
        let update = self.store.read().unwrap().diff_state_vector(sv)?;

        let mut encoder = RawEncoder::default();
        update.write(&mut encoder)?;
        Ok(encoder.into_inner())
    }

    pub fn get_state_vector(&self) -> StateVector {
        self.store.read().unwrap().get_state_vector()
    }

    pub fn subscribe(&self, cb: impl Fn(&[u8]) + Sync + Send + 'static) {
        self.publisher.subscribe(cb);
    }

    pub fn unsubscribe_all(&self) {
        self.publisher.unsubscribe_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{AtomicU8, Ordering};
    use yrs::{types::ToJson, updates::decoder::Decode, Array, Map, Options, Transact};

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_double_run_with_yrs_basic() {
        let yrs_doc = yrs::Doc::new();

        let map = yrs_doc.get_or_insert_map("abc");
        let mut trx = yrs_doc.transact_mut();
        map.insert(&mut trx, "a", 1).unwrap();

        let binary_from_yrs = trx.encode_update_v1().unwrap();

        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        loom_model!({
            let doc = Doc::new_from_binary_with_options(binary_from_yrs.clone(), options.clone())
                .unwrap();
            let binary = doc.encode_update_v1().unwrap();

            assert_eq!(binary_from_yrs, binary);
        });
    }

    #[test]
    fn test_encode_state_as_update() {
        let options_left = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };
        let options_right = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        let yrs_options_left = Options {
            client_id: rand::random(),
            guid: nanoid::nanoid!().into(),
            ..Default::default()
        };

        let yrs_options_right = Options {
            client_id: rand::random(),
            guid: nanoid::nanoid!().into(),
            ..Default::default()
        };

        loom_model!({
            let (binary, binary_new) = if cfg!(miri) {
                let doc = Doc::with_options(options_left.clone());

                let mut map = doc.get_or_create_map("abc").unwrap();
                map.insert("a", 1).unwrap();
                let binary = doc.encode_update_v1().unwrap();

                let doc_new = Doc::with_options(options_right.clone());
                let mut array = doc_new.get_or_create_array("array").unwrap();
                array.insert(0, "array_value").unwrap();
                let binary_new = doc.encode_update_v1().unwrap();

                (binary, binary_new)
            } else {
                let yrs_doc = yrs::Doc::with_options(yrs_options_left.clone());

                let map = yrs_doc.get_or_insert_map("abc");
                let mut trx = yrs_doc.transact_mut();
                map.insert(&mut trx, "a", 1).unwrap();
                let binary = trx.encode_update_v1().unwrap();

                let yrs_doc_new = yrs::Doc::with_options(yrs_options_right.clone());
                let array = yrs_doc_new.get_or_insert_array("array");
                let mut trx = yrs_doc_new.transact_mut();
                array.insert(&mut trx, 0, "array_value").unwrap();
                let binary_new = trx.encode_update_v1().unwrap();

                (binary, binary_new)
            };

            let mut doc =
                Doc::new_from_binary_with_options(binary.clone(), options_left.clone()).unwrap();
            let mut doc_new =
                Doc::new_from_binary_with_options(binary_new.clone(), options_right.clone())
                    .unwrap();

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
        });
    }

    #[test]
    #[cfg_attr(any(miri, loom), ignore)]
    fn test_array_create() {
        let options = DocOptions {
            client: Some(rand::random()),
            guid: Some(nanoid::nanoid!()),
        };

        let yrs_options = yrs::Options::with_guid_and_client_id(
            options.guid.clone().unwrap().into(),
            options.client.unwrap(),
        );

        let json = serde_json::json!([42.0, -42.0, true, false, "hello", "world", [1.0]]);

        {
            let doc = yrs::Doc::with_options(yrs_options.clone());
            let array = doc.get_or_insert_array("abc");
            let mut trx = doc.transact_mut();
            array.insert(&mut trx, 0, 42).unwrap();
            array.insert(&mut trx, 1, -42).unwrap();
            array.insert(&mut trx, 2, true).unwrap();
            array.insert(&mut trx, 3, false).unwrap();
            array.insert(&mut trx, 4, "hello").unwrap();
            array.insert(&mut trx, 5, "world").unwrap();

            let sub_array = yrs::ArrayPrelim::default();
            let sub_array = array.insert(&mut trx, 6, sub_array).unwrap();
            sub_array.insert(&mut trx, 0, 1).unwrap();

            drop(trx);

            assert_json_diff::assert_json_eq!(array.to_json(&doc.transact()), json);
        };

        let binary = {
            let doc = Doc::with_options(options.clone());
            let mut array = doc.get_or_create_array("abc").unwrap();
            array.insert(0, 42).unwrap();
            array.insert(1, -42).unwrap();
            array.insert(2, true).unwrap();
            array.insert(3, false).unwrap();
            array.insert(4, "hello").unwrap();
            array.insert(5, "world").unwrap();

            let mut sub_array = doc.create_array().unwrap();
            array.insert(6, sub_array.clone()).unwrap();
            // FIXME: array need insert first to compatible with yrs
            sub_array.insert(0, 1).unwrap();

            doc.encode_update_v1().unwrap()
        };

        let ydoc = yrs::Doc::with_options(yrs_options);
        let array = ydoc.get_or_insert_array("abc");
        let mut trx = ydoc.transact_mut();
        trx.apply_update(yrs::Update::decode_v1(&binary).unwrap());

        assert_json_diff::assert_json_eq!(array.to_json(&trx), json);

        let mut doc = Doc::with_options(options);
        let array = doc.get_or_create_array("abc").unwrap();
        doc.apply_update_from_binary(binary).unwrap();

        let list = array.iter().collect::<Vec<_>>();

        assert!(list.len() == 7);
        assert!(matches!(list[6], Value::Array(_)));
    }

    #[test]
    #[ignore = "inaccurate timing on ci, need for more accurate timing testing"]
    fn test_subscribe() {
        loom_model!({
            let doc = Doc::default();
            let doc_clone = doc.clone();

            let count = Arc::new(AtomicU8::new(0));
            let count_clone1 = count.clone();
            let count_clone2 = count.clone();
            doc.subscribe(move |_| {
                count_clone1.fetch_add(1, Ordering::SeqCst);
            });

            doc_clone.subscribe(move |_| {
                count_clone2.fetch_add(1, Ordering::SeqCst);
            });

            doc_clone
                .get_or_create_array("abc")
                .unwrap()
                .insert(0, 42)
                .unwrap();

            // wait observer, cycle once every 100mm
            std::thread::sleep(std::time::Duration::from_millis(200));

            assert_eq!(count.load(Ordering::SeqCst), 2);
        });
    }
}
