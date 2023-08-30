use std::collections::HashMap;

use super::{publisher::DocPublisher, store::StoreRef, *};
use crate::sync::{Arc, RwLock};

/// [DocOptions] used to create a new [Doc]
///
/// ```
/// let doc = DocOptions::new()
///     .with_client_id(1)
///     .with_guid("guid".into())
///     .auto_gc(true)
///     .build();
///
/// assert!(doc.guid(), "guid")
/// ```
#[derive(Clone, Debug)]
pub struct DocOptions {
    pub guid: String,
    pub client_id: u64,
    pub gc: bool,
}

impl Default for DocOptions {
    fn default() -> Self {
        if cfg!(test) {
            Self {
                client_id: 1,
                guid: "test".into(),
                gc: true,
            }
        } else {
            Self {
                client_id: rand::random(),
                guid: nanoid::nanoid!(),
                gc: true,
            }
        }
    }
}

impl DocOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client_id(mut self, client_id: u64) -> Self {
        self.client_id = client_id;
        self
    }

    pub fn with_guid(mut self, guid: String) -> Self {
        self.guid = guid;
        self
    }

    pub fn auto_gc(mut self, gc: bool) -> Self {
        self.gc = gc;
        self
    }

    pub fn build(self) -> Doc {
        Doc::with_options(self)
    }
}

impl From<DocOptions> for Any {
    fn from(value: DocOptions) -> Self {
        Any::Object(HashMap::from([
            ("gc".into(), value.gc.into()),
            ("guid".into(), value.guid.into()),
        ]))
    }
}

impl TryFrom<Any> for DocOptions {
    type Error = JwstCodecError;

    fn try_from(value: Any) -> Result<Self, Self::Error> {
        match value {
            Any::Object(map) => {
                let mut options = DocOptions::default();
                for (key, value) in map {
                    match key.as_str() {
                        "gc" => {
                            options.gc = bool::try_from(value)?;
                        }
                        "guid" => {
                            options.guid = String::try_from(value)?;
                        }
                        _ => {}
                    }
                }

                Ok(options)
            }
            _ => Err(JwstCodecError::UnexpectedType("Object")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Doc {
    client_id: u64,
    opts: DocOptions,

    pub(crate) store: StoreRef,
    pub(crate) publisher: Arc<DocPublisher>,
}

unsafe impl Send for Doc {}
unsafe impl Sync for Doc {}

impl Default for Doc {
    fn default() -> Self {
        Doc::new()
    }
}

impl PartialEq for Doc {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
    }
}

impl Doc {
    pub fn new() -> Self {
        Self::with_options(DocOptions::default())
    }

    pub fn with_options(options: DocOptions) -> Self {
        let store = Arc::new(RwLock::new(DocStore::with_client(options.client_id)));
        let publisher = Arc::new(DocPublisher::new(store.clone()));

        Self {
            client_id: options.client_id,
            opts: options,
            store,
            publisher,
        }
    }

    pub fn with_client(client_id: u64) -> Self {
        DocOptions::new().with_client_id(client_id).build()
    }

    pub fn client(&self) -> Client {
        self.client_id
    }

    pub fn options(&self) -> &DocOptions {
        &self.opts
    }

    pub fn guid(&self) -> &str {
        self.opts.guid.as_str()
    }

    pub fn new_from_binary(binary: Vec<u8>) -> JwstCodecResult<Self> {
        let mut doc = Doc::default();
        doc.apply_update_from_binary(binary)?;
        Ok(doc)
    }

    pub fn new_from_binary_with_options(binary: Vec<u8>, options: DocOptions) -> JwstCodecResult<Self> {
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
                    let mut item = unsafe { item.get_mut_unchecked() };
                    store.repair(&mut item, self.store.clone())?;
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
                    Update::merge_into(&mut update, [pending_update]);
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
                    retry = false;
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
        YTypeBuilder::new(self.store.clone()).with_kind(YTypeKind::Text).build()
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
        YTypeBuilder::new(self.store.clone()).with_kind(YTypeKind::Map).build()
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

    pub fn gc(&self) -> JwstCodecResult<()> {
        self.store.write().unwrap().optimize()
    }
}

#[cfg(test)]
mod tests {
    use yrs::{types::ToJson, updates::decoder::Decode, Array, Map, Options, Transact};

    use super::*;
    use crate::sync::{AtomicU8, Ordering};

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_double_run_with_yrs_basic() {
        let yrs_doc = yrs::Doc::new();

        let map = yrs_doc.get_or_insert_map("abc");
        let mut trx = yrs_doc.transact_mut();
        map.insert(&mut trx, "a", 1).unwrap();

        let binary_from_yrs = trx.encode_update_v1().unwrap();

        loom_model!({
            let doc = Doc::new_from_binary(binary_from_yrs.clone()).unwrap();
            let binary = doc.encode_update_v1().unwrap();

            assert_eq!(binary_from_yrs, binary);
        });
    }

    #[test]
    fn test_encode_state_as_update() {
        let yrs_options_left = Options::default();
        let yrs_options_right = Options::default();

        loom_model!({
            let (binary, binary_new) = if cfg!(miri) {
                let doc = Doc::new();

                let mut map = doc.get_or_create_map("abc").unwrap();
                map.insert("a", 1).unwrap();
                let binary = doc.encode_update_v1().unwrap();

                let doc_new = Doc::new();
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

            let mut doc = Doc::new_from_binary(binary.clone()).unwrap();
            let mut doc_new = Doc::new_from_binary(binary_new.clone()).unwrap();

            let diff_update = doc_new.encode_state_as_update_v1(&doc.get_state_vector()).unwrap();

            let diff_update_reverse = doc.encode_state_as_update_v1(&doc_new.get_state_vector()).unwrap();

            doc.apply_update_from_binary(diff_update).unwrap();
            doc_new.apply_update_from_binary(diff_update_reverse).unwrap();

            assert_eq!(doc.encode_update_v1().unwrap(), doc_new.encode_update_v1().unwrap());
        });
    }

    #[test]
    #[cfg_attr(any(miri, loom), ignore)]
    fn test_array_create() {
        let yrs_options = yrs::Options::default();

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
            let doc = Doc::new();
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

        let mut doc = Doc::new();
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

            doc_clone.get_or_create_array("abc").unwrap().insert(0, 42).unwrap();

            // wait observer, cycle once every 100mm
            std::thread::sleep(std::time::Duration::from_millis(200));

            assert_eq!(count.load(Ordering::SeqCst), 2);
        });
    }
}
