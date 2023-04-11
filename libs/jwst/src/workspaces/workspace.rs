use super::plugins::{setup_plugin, PluginMap};
use serde::{ser::SerializeMap, Serialize, Serializer};
use std::{collections::HashMap, sync::Arc};
use std::collections::HashSet;
use anyhow::Context;
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::sync::{RwLock};
use y_sync::awareness::Awareness;
use yrs::{
    types::{map::MapEvent, ToJson},
    Doc, Map, MapRef, Subscription, Transact, TransactionMut, UpdateSubscription,
};

pub type MapSubscription = Subscription<Arc<dyn Fn(&TransactionMut, &MapEvent)>>;

pub struct Workspace {
    workspace_id: String,
    pub(super) awareness: Arc<RwLock<Awareness>>,
    pub(super) doc: Doc,
    pub(super) sub: Arc<RwLock<HashMap<String, UpdateSubscription>>>,
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
    // pub(crate) rx: Arc<Mutex<std::sync::mpsc::Receiver<String>>>,
    // pub(crate) handle: Arc<RwLock<Option<JoinHandle<()>>>>,
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
        updated: MapRef,
        metadata: MapRef,
        plugins: PluginMap,
    ) -> Workspace {
        Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness,
            doc,
            sub,
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
        // rx: Arc::new(Mutex::new(rx)),
        // handle: Arc::new(RwLock::new(None)),
    }));

    // TODO: lazy start block callback thread
    {
        std::thread::spawn(move || {
            let rt = runtime.clone();
            while let Ok(block_id) = rx.recv() {
                let modified_block_ids = modified_block_ids.clone();
                let callback = callback.clone();
                rt.spawn(async move {
                    if let Some(callback) = callback.read().await.as_ref() {
                        let mut guard = modified_block_ids.write().await;
                        guard.insert(block_id);
                        // TODO: merge recent block_id
                        let block_ids = guard.iter().map(|item| item.to_owned()).collect::<Vec<String>>();
                        callback(block_ids);
                        guard.clear();
                    }
                });
            }
        });
    }

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
            self.updated.clone(),
            self.metadata.clone(),
            self.plugins.clone(),
        )
    }
}

#[cfg(test)]
mod test {
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
}
