use super::plugins::{setup_plugin, PluginMap};
use serde::{ser::SerializeMap, Serialize, Serializer};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use y_sync::awareness::{Awareness, Event, Subscription as AwarenessSubscription};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration};
use anyhow::Context;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::time::sleep;
use tracing::{debug};
use yrs::{
    types::{map::MapEvent, ToJson},
    Doc, Map, MapRef, Subscription, Transact, TransactionMut, UpdateSubscription,
};

pub type MapSubscription = Subscription<Arc<dyn Fn(&TransactionMut, &MapEvent)>>;

pub struct Workspace {
    workspace_id: String,
    pub(super) awareness: Arc<RwLock<Awareness>>,
    pub(super) doc: Doc,
    // TODO: Unreasonable subscription mechanism, needs refactoring
    pub(super) sub: Arc<RwLock<HashMap<String, UpdateSubscription>>>,
    pub(super) awareness_sub: Arc<Option<AwarenessSubscription<Event>>>,
    pub(crate) updated: MapRef,
    pub(crate) metadata: MapRef,
    /// We store plugins so that their ownership is tied to [Workspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [Workspace] experiences, like block updates.
    ///
    /// Public just for the crate as we experiment with the plugins interface.
    /// See [super::plugins].
    pub(super) plugins: PluginMap,
    pub(crate) block_observer_config: Option<Arc<BlockObserverConfig>>,
}

pub struct BlockObserverConfig {
    pub(crate) callback: Arc<RwLock<Option<Box<dyn Fn(Vec<String>) -> () + Send +Sync>>>>,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) tx: std::sync::mpsc::Sender<String>,
    pub(crate) rx: Arc<Mutex<std::sync::mpsc::Receiver<String>>>,
    pub(crate) modified_block_ids: Arc<RwLock<HashSet<String>>>,
    pub(crate) handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

unsafe impl Send for Workspace {}
unsafe impl Sync for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        let doc = Doc::new();
        Self::from_doc(doc, id)
    }

    pub fn set_callback(&self, cb: Box<dyn Fn(Vec<String>) + Send + Sync>) {
        if let Some(block_observer_config) = self.block_observer_config.clone() {
            let callback = block_observer_config.callback.clone();
            block_observer_config.runtime.spawn(async move {
                *callback.write().await = Some(cb);
            });

            let mut handle_guard = block_observer_config.handle.lock().unwrap();
            if let None = *handle_guard {
                let runtime = block_observer_config.runtime.clone();
                let modified_block_ids = block_observer_config.modified_block_ids.clone();
                let rx = block_observer_config.rx.clone();
                let callback = block_observer_config.callback.clone();
                *handle_guard = Some({
                    std::thread::spawn(move || {
                        let rx = rx.lock().unwrap();
                        let rt = runtime.clone();
                        while let Ok(block_id) = rx.recv() {
                            debug!("received block change from {}", block_id);
                            let modified_block_ids = modified_block_ids.clone();
                            let callback = callback.clone();
                            rt.spawn(async move {
                                if let Some(callback) = callback.read().await.as_ref() {
                                    let mut guard = modified_block_ids.write().await;
                                    guard.insert(block_id);
                                    drop(guard);
                                    // merge changed blocks in between 200 ms
                                    sleep(Duration::from_millis(200)).await;
                                    let mut guard = modified_block_ids.write().await;
                                    if !guard.is_empty() {
                                        let block_ids = guard.iter().map(|item| item.to_owned()).collect::<Vec<String>>();
                                        debug!("invoking callback with block ids: {:?}", block_ids);
                                        callback(block_ids);
                                        guard.clear();
                                    }
                                }
                            });
                        }
                    })
                })
            }
        }
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, workspace_id: S) -> Workspace {
        let updated = doc.get_or_insert_map("space:updated");
        let metadata = doc.get_or_insert_map("space:meta");

        setup_plugin(Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness: Arc::new(RwLock::new(Awareness::new(doc.clone()))),
            doc,
            sub: Arc::default(),
            awareness_sub: Arc::default(),
            updated,
            metadata,
            plugins: Default::default(),
            block_observer_config: generate_block_observer_config(),
        })
    }

    fn from_raw<S: AsRef<str>>(
        workspace_id: S,
        awareness: Arc<RwLock<Awareness>>,
        doc: Doc,
        sub: Arc<RwLock<HashMap<String, UpdateSubscription>>>,
        awareness_sub: Arc<Option<AwarenessSubscription<Event>>>,
        updated: MapRef,
        metadata: MapRef,
        plugins: PluginMap,
    ) -> Workspace {
        Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness,
            doc,
            sub,
            awareness_sub,
            updated,
            metadata,
            plugins,
            block_observer_config: generate_block_observer_config(),
        }
    }

    pub fn is_empty(&self) -> bool {
        let doc = self.doc();
        let trx = doc.transact();
        self.updated.len(&trx) == 0
    }

    pub fn id(&self) -> String {
        self.workspace_id.clone()
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client_id()
    }

    pub fn doc(&self) -> Doc {
        self.doc.clone()
    }
}

fn generate_block_observer_config() -> Option<Arc<BlockObserverConfig>> {
    let runtime = Arc::new(runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .context("Failed to create runtime")
        .unwrap());
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let modified_block_ids = Arc::new(RwLock::new(HashSet::new()));
    let callback = Arc::new(RwLock::new(None));
    let block_observer_config = Some(Arc::new(BlockObserverConfig {
        callback: callback.clone(),
        runtime: runtime.clone(),
        tx,
        rx: Arc::new(Mutex::new(rx)),
        modified_block_ids,
        handle: Arc::new(Mutex::new(None)),
    }));

    block_observer_config
}

impl Serialize for Workspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;

        for space in self.with_trx(|t| t.spaces(|spaces| spaces.collect::<Vec<_>>())) {
            map.serialize_entry(&format!("space:{}", space.space_id()), &space)?;
        }

        let trx = self.doc.transact();
        map.serialize_entry("space:meta", &self.metadata.to_json(&trx))?;
        map.serialize_entry("space:updated", &self.updated.to_json(&trx))?;

        map.end()
    }
}

impl Clone for Workspace {
    fn clone(&self) -> Self {
        Self::from_raw(
            &self.workspace_id,
            self.awareness.clone(),
            self.doc.clone(),
            self.sub.clone(),
            self.awareness_sub.clone(),
            self.updated.clone(),
            self.metadata.clone(),
            self.plugins.clone(),
        )
    }
}

#[cfg(test)]
mod test {
    use std::thread::sleep;
    use super::{super::super::Block, *};
    use tracing::info;
    use yrs::{updates::decoder::Decode, Doc, Map, ReadTxn, StateVector, Update};

    #[test]
    fn doc_load_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let space = t.get_space("test");

            let block = space.create(&mut t.trx, "test", "text").unwrap();

            block.set(&mut t.trx, "test", "test").unwrap();
        });

        let doc = workspace.doc();

        let new_doc = {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());
            let doc = Doc::default();
            {
                let mut trx = doc.transact_mut();
                match update.and_then(|update| Update::decode_v1(&update)) {
                    Ok(update) => trx.apply_update(update),
                    Err(err) => info!("failed to decode update: {:?}", err),
                }
                trx.commit();
            }
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.get_or_insert_map("space:meta").to_json(&doc.transact()),
            new_doc
                .get_or_insert_map("space:meta")
                .to_json(&doc.transact())
        );

        assert_json_diff::assert_json_eq!(
            doc.get_or_insert_map("space:updated")
                .to_json(&doc.transact()),
            new_doc
                .get_or_insert_map("space:updated")
                .to_json(&doc.transact())
        );
    }

    #[test]
    fn block_observe_callback() {
        let workspace = Workspace::new("test");
        workspace.set_callback(Box::new(|block_ids| assert_eq!(block_ids, vec!["block1".to_string(), "block2".to_string()])));

        let (block1, block2) = workspace.with_trx(|mut t| {
            let space = t.get_space("test");
            let block1 = space.create(&mut t.trx, "block1", "text").unwrap();
            let block2 = space.create(&mut t.trx, "block2", "text").unwrap();
            (block1, block2)
        });

        workspace.with_trx(|mut trx| {
            block1.set(&mut trx.trx, "key1", "value1").unwrap();
            block1.set(&mut trx.trx, "key2", "value2").unwrap();
            block2.set(&mut trx.trx, "key1", "value1").unwrap();
            block2.set(&mut trx.trx, "key2", "value2").unwrap();
        });
        sleep(Duration::from_millis(300));
    }

    #[test]
    fn workspace() {
        let workspace = Workspace::new("test");

        workspace.with_trx(|t| {
            assert_eq!(workspace.id(), "test");
            assert_eq!(workspace.updated.len(&t.trx), 0);
        });

        workspace.with_trx(|mut t| {
            let space = t.get_space("test");

            let block = space.create(&mut t.trx, "block", "text").unwrap();

            assert_eq!(space.blocks.len(&t.trx), 1);
            assert_eq!(workspace.updated.len(&t.trx), 1);
            assert_eq!(block.block_id(), "block");
            assert_eq!(block.flavour(&t.trx), "text");

            assert_eq!(
                space.get(&t.trx, "block").map(|b| b.block_id()),
                Some("block".to_owned())
            );

            assert!(space.exists(&t.trx, "block"));

            assert!(space.remove(&mut t.trx, "block"));

            assert_eq!(space.blocks.len(&t.trx), 0);
            assert_eq!(workspace.updated.len(&t.trx), 0);
            assert_eq!(space.get(&t.trx, "block"), None);
            assert!(!space.exists(&t.trx, "block"));
        });

        workspace.with_trx(|mut t| {
            let space = t.get_space("test");

            Block::new(&mut t.trx, &space, "test", "test", 1).unwrap();
            let vec = space.get_blocks_by_flavour(&t.trx, "test");
            assert_eq!(vec.len(), 1);
        });

        let doc = Doc::with_client_id(123);
        let workspace = Workspace::from_doc(doc, "test");
        assert_eq!(workspace.client_id(), 123);
    }

    #[test]
    fn workspace_struct() {
        use assert_json_diff::assert_json_include;

        let workspace = Workspace::new("workspace");

        workspace.with_trx(|mut t| {
            let space = t.get_space("space1");
            space.create(&mut t.trx, "block1", "text").unwrap();

            let space = t.get_space("space2");
            space.create(&mut t.trx, "block2", "text").unwrap();
        });

        assert_json_include!(
            actual: serde_json::to_value(&workspace).unwrap(),
            expected: serde_json::json!({
                "space:space1": {
                    "block1": {
                        "sys:children": [],
                        "sys:flavour": "text",
                    }
                },
                "space:space2": {
                    "block2": {
                        "sys:children": [],
                        "sys:flavour": "text",
                    }
                },
                "space:updated": {
                    "block1": [[]],
                    "block2": [[]],
                },
                "space:meta": {}
            })
        );
    }

    #[test]
    fn scan_doc() {
        let doc = Doc::new();
        let map = doc.get_or_insert_map("test");
        map.insert(&mut doc.transact_mut(), "test", "aaa").unwrap();

        let data = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
            .unwrap();

        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(Update::decode_v1(&data).unwrap());

        assert_eq!(doc.transact().store().root_keys(), vec!["test"]);
    }

    #[test]
    fn test_same_id_type_merge() {
        let update = {
            let doc = Doc::new();
            let ws = Workspace::from_doc(doc, "test");
            ws.with_trx(|mut t| {
                let space = t.get_space("space");
                let _block = space.create(&mut t.trx, "test", "test1").unwrap();
            });

            ws.doc()
                .transact()
                .encode_state_as_update_v1(&StateVector::default())
                .unwrap()
        };
        let update1 = {
            let doc = Doc::new();
            doc.transact_mut()
                .apply_update(Update::decode_v1(&update).unwrap());
            let ws = Workspace::from_doc(doc, "test");
            ws.with_trx(|mut t| {
                let space = t.get_space("space");
                let new_block = space.create(&mut t.trx, "test1", "test1").unwrap();
                let block = space.get(&mut t.trx, "test").unwrap();
                block.insert_children_at(&mut t.trx, &new_block, 0).unwrap();
            });

            ws.doc()
                .transact()
                .encode_state_as_update_v1(&StateVector::default())
                .unwrap()
        };
        let update2 = {
            let doc = Doc::new();
            doc.transact_mut()
                .apply_update(Update::decode_v1(&update).unwrap());
            let ws = Workspace::from_doc(doc, "test");
            ws.with_trx(|mut t| {
                let space = t.get_space("space");
                let new_block = space.create(&mut t.trx, "test2", "test2").unwrap();
                let block = space.get(&mut t.trx, "test").unwrap();
                block.insert_children_at(&mut t.trx, &new_block, 0).unwrap();
            });

            ws.doc()
                .transact()
                .encode_state_as_update_v1(&StateVector::default())
                .unwrap()
        };

        // let merged_update = yrs::merge_updates_v1(&[&update1, &update2]).unwrap();

        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(Update::decode_v1(&update1).unwrap());
        doc.transact_mut()
            .apply_update(Update::decode_v1(&update2).unwrap());

        let ws = Workspace::from_doc(doc, "test");
        let block = ws.with_trx(|mut t| {
            let space = t.get_space("space");
            space.get(&t.trx, "test").unwrap()
        });
        println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

        ws.with_trx(|mut t| {
            let space = t.get_space("space");
            let block = space.get(&t.trx, "test").unwrap();
            assert_eq!(
                block.children(&t.trx),
                vec!["test2".to_owned(), "test1".to_owned()]
            );
            // assert_eq!(block.get(&t.trx, "test1").unwrap().to_string(), "test1");
            // assert_eq!(block.get(&t.trx, "test2").unwrap().to_string(), "test2");
        });
    }
}
