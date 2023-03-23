use super::{metadata::SEARCH_INDEX, plugins::setup_plugin, *};
use serde::{ser::SerializeMap, Serialize, Serializer};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
    thread::sleep,
    time::Duration,
};
use tokio::sync::RwLock;
use y_sync::{
    awareness::{Awareness, Event, Subscription as AwarenessSubscription},
    sync::{DefaultProtocol, Error, Message, MessageReader, Protocol, SyncMessage},
};
use yrs::{
    types::{map::MapEvent, ToJson},
    updates::{
        decoder::{Decode, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
    Doc, MapRef, Observable, ReadTxn, StateVector, Subscription, Transact, TransactionMut, Update,
    UpdateEvent, UpdateSubscription,
};

static PROTOCOL: DefaultProtocol = DefaultProtocol;

use super::PluginMap;
use plugins::PluginImpl;

pub type MapSubscription = Subscription<Arc<dyn Fn(&TransactionMut, &MapEvent)>>;

pub struct Workspace {
    workspace_id: String,
    awareness: Arc<RwLock<Awareness>>,
    doc: Doc,
    pub(crate) updated: MapRef,
    pub(crate) metadata: MapRef,
    /// We store plugins so that their ownership is tied to [Workspace].
    /// This enables us to properly manage lifetimes of observers which will subscribe
    /// into events that the [Workspace] experiences, like block updates.
    ///
    /// Public just for the crate as we experiment with the plugins interface.
    /// See [plugins].
    pub(super) plugins: PluginMap,
}

unsafe impl Send for Workspace {}
unsafe impl Sync for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> Self {
        let doc = Doc::new();
        Self::from_doc(doc, id)
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, workspace_id: S) -> Workspace {
        let updated = doc.get_or_insert_map("space:updated");
        let metadata = doc.get_or_insert_map("space:meta");

        setup_plugin(Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness: Arc::new(RwLock::new(Awareness::new(doc.clone()))),
            doc,
            updated,
            metadata,
            plugins: Default::default(),
        })
    }

    fn from_raw<S: AsRef<str>>(
        workspace_id: S,
        awareness: Arc<RwLock<Awareness>>,
        doc: Doc,
        updated: MapRef,
        metadata: MapRef,
        plugins: PluginMap,
    ) -> Workspace {
        Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness,
            doc,
            updated,
            metadata,
            plugins,
        }
    }

    /// Allow the plugin to run any necessary updates it could have flagged via observers.
    /// See [plugins].
    pub(super) fn update_plugin<P: PluginImpl>(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.plugins.update_plugin::<P>(self)
    }

    /// See [plugins].
    pub(super) fn with_plugin<P: PluginImpl, T>(&self, cb: impl Fn(&P) -> T) -> Option<T> {
        self.plugins.with_plugin::<P, T>(cb)
    }

    #[cfg(feature = "workspace-search")]
    pub fn search<S: AsRef<str>>(
        &self,
        query: S,
    ) -> Result<SearchResults, Box<dyn std::error::Error>> {
        use plugins::IndexingPluginImpl;

        // refresh index if doc has update
        self.update_plugin::<IndexingPluginImpl>()?;

        let query = query.as_ref();

        self.with_plugin::<IndexingPluginImpl, Result<SearchResults, Box<dyn std::error::Error>>>(
            |search_plugin| search_plugin.search(query),
        )
        .expect("text search was set up by default")
    }

    pub fn search_result(&self, query: String) -> String {
        match self.search(query) {
            Ok(list) => serde_json::to_string(&list).unwrap(),
            Err(_) => "[]".to_string(),
        }
    }

    pub fn set_search_index(&self, fields: Vec<String>) -> bool {
        match fields.iter().find(|&field| field.is_empty()) {
            Some(field) => {
                error!("field name cannot be empty: {}", field);
                false
            }
            None => {
                let value = serde_json::to_string(&fields).unwrap();
                self.with_trx(|mut trx| trx.set_metadata(SEARCH_INDEX, value));
                setup_plugin(self.clone());
                true
            }
        }
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> T {
        let doc = self.doc();
        let trx = WorkspaceTransaction {
            trx: doc.transact_mut(),
            ws: self,
        };

        f(trx)
    }

    pub fn try_with_trx<T>(&self, f: impl FnOnce(WorkspaceTransaction) -> T) -> Option<T> {
        match self.doc().try_transact_mut() {
            Ok(trx) => {
                let trx = WorkspaceTransaction { trx, ws: self };
                Some(f(trx))
            }
            Err(e) => {
                info!("try_with_trx error: {}", e);
                None
            }
        }
    }

    pub fn retry_with_trx<T>(
        &self,
        f: impl FnOnce(WorkspaceTransaction) -> T,
        mut retry: i32,
    ) -> Option<T> {
        let trx = loop {
            if let Ok(trx) = self.doc.try_transact_mut() {
                break trx;
            } else if retry > 0 {
                retry -= 1;
                sleep(Duration::from_micros(10));
            } else {
                info!("retry_with_trx error");
                return None;
            }
        };

        Some(f(WorkspaceTransaction { trx, ws: self }))
    }

    pub fn id(&self) -> String {
        self.workspace_id.clone()
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client_id()
    }

    pub fn metadata(&self) -> WorkspaceMetadata {
        (&self.doc().transact(), self.metadata.clone()).into()
    }

    pub fn observe_metadata(
        &mut self,
        f: impl Fn(&TransactionMut, &MapEvent) + 'static,
    ) -> MapSubscription {
        self.metadata.observe(f)
    }

    pub async fn on_awareness_update(
        &mut self,
        f: impl Fn(&Awareness, &Event) + 'static,
    ) -> AwarenessSubscription<Event> {
        self.awareness.write().await.on_update(f)
    }

    /// Subscribe to update events.
    pub fn observe(
        &mut self,
        f: impl Fn(&TransactionMut, &UpdateEvent) + Clone + 'static,
    ) -> Option<UpdateSubscription> {
        info!("workspace observe enter");
        let doc = self.doc();
        match catch_unwind(AssertUnwindSafe(move || {
            let mut retry = 10;
            loop {
                let f = f.clone();
                match doc.observe_update_v1(move |trx, evt| {
                    trace!("workspace observe: observe_update_v1, {:?}", &evt.update);
                    if let Err(e) = catch_unwind(AssertUnwindSafe(|| f(trx, evt))) {
                        error!("panic in observe callback: {:?}", e);
                    }
                }) {
                    Ok(sub) => break Ok(sub),
                    Err(e) if retry <= 0 => break Err(e),
                    _ => {
                        sleep(Duration::from_micros(100));
                        retry -= 1;
                        continue;
                    }
                }
            }
        })) {
            Ok(sub) => sub.map_err(|e| error!("failed to observe: {:?}", e)).ok(),
            Err(e) => {
                error!("panic in observe callback: {:?}", e);
                None
            }
        }
    }

    pub fn doc(&self) -> Doc {
        self.doc.clone()
    }

    pub fn sync_migration(&self, mut retry: i32) -> Option<Vec<u8>> {
        let trx = loop {
            if let Ok(trx) = self.doc.try_transact() {
                break trx;
            } else if retry > 0 {
                retry -= 1;
                sleep(Duration::from_micros(10));
            } else {
                return None;
            }
        };
        Some(trx.encode_state_as_update_v1(&StateVector::default()))
    }

    pub async fn sync_init_message(&self) -> Result<Vec<u8>, Error> {
        let mut encoder = EncoderV1::new();
        PROTOCOL.start(&*self.awareness.read().await, &mut encoder)?;
        Ok(encoder.to_vec())
    }

    pub async fn sync_decode_message(&mut self, binary: &[u8]) -> Vec<Vec<u8>> {
        let mut decoder = DecoderV1::from(binary);
        let mut result = vec![];

        let (awareness_msg, content_msg): (Vec<_>, Vec<_>) = MessageReader::new(&mut decoder)
            .flatten()
            .partition(|msg| matches!(msg, Message::Awareness(_) | Message::AwarenessQuery));

        if !awareness_msg.is_empty() {
            let mut awareness = self.awareness.write().await;
            if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
                for msg in awareness_msg {
                    match msg {
                        Message::AwarenessQuery => {
                            if let Ok(update) = awareness.update() {
                                result.push(Message::Awareness(update).encode_v1());
                            }
                        }
                        Message::Awareness(update) => {
                            if let Err(e) = awareness.apply_update(update) {
                                warn!("failed to apply awareness: {:?}", e);
                            }
                        }
                        _ => {}
                    }
                }
            })) {
                warn!("failed to apply awareness update: {:?}", e);
            }
        }
        if !content_msg.is_empty() {
            let doc = self.doc();
            if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
                let mut retry = 30;
                let mut trx = loop {
                    if let Ok(trx) = doc.try_transact_mut() {
                        break trx;
                    } else if retry > 0 {
                        retry -= 1;
                        sleep(Duration::from_micros(10));
                    } else {
                        return;
                    }
                };
                for msg in content_msg {
                    if let Some(msg) = {
                        trace!("processing message: {:?}", msg);
                        match msg {
                            Message::Sync(msg) => match msg {
                                SyncMessage::SyncStep1(sv) => {
                                    let update = trx.encode_state_as_update_v1(&sv);
                                    Some(Message::Sync(SyncMessage::SyncStep2(update)))
                                }
                                SyncMessage::SyncStep2(update) => {
                                    if let Ok(update) = Update::decode_v1(&update) {
                                        trx.apply_update(update);
                                    }
                                    None
                                }
                                SyncMessage::Update(update) => {
                                    if let Ok(update) = Update::decode_v1(&update) {
                                        trx.apply_update(update);
                                        trx.commit();
                                        if cfg!(debug_assertions) {
                                            trace!(
                                                "changed_parent_types: {:?}",
                                                trx.changed_parent_types()
                                            );
                                            trace!("before_state: {:?}", trx.before_state());
                                            trace!("after_state: {:?}", trx.after_state());
                                        }
                                        let update = trx.encode_update_v1();
                                        Some(Message::Sync(SyncMessage::Update(update)))
                                    } else {
                                        None
                                    }
                                }
                            },
                            _ => None,
                        }
                    } {
                        result.push(msg.encode_v1());
                    }
                }
            })) {
                warn!("failed to apply update: {:?}", e);
            }
        }

        result
    }
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
    use yrs::{updates::decoder::Decode, Doc, Map, StateVector, Update};

    #[test]
    fn doc_load_test() {
        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let space = t.get_space("test");

            let block = space.create(&mut t.trx, "test", "text");

            block.set(&mut t.trx, "test", "test");
        });

        let doc = workspace.doc();

        let new_doc = {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default());
            let doc = Doc::default();
            {
                let mut trx = doc.transact_mut();
                match Update::decode_v1(&update) {
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

            let block = space.create(&mut t.trx, "block", "text");

            assert_eq!(space.blocks.len(&t.trx), 1);
            assert_eq!(workspace.updated.len(&t.trx), 1);
            assert_eq!(block.block_id(), "block");
            assert_eq!(block.flavor(&t.trx), "text");

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

            Block::new(&mut t.trx, &space, "test", "test", 1);
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
            space.create(&mut t.trx, "block1", "text");

            let space = t.get_space("space2");
            space.create(&mut t.trx, "block2", "text");
        });

        assert_json_include!(
            actual: serde_json::to_value(&workspace).unwrap(),
            expected: serde_json::json!({
                "space:space1": {
                    "block1": {
                        "sys:children": [],
                        "sys:flavor": "text",
                        "sys:version": [1.0, 0.0],
                    }
                },
                "space:space2": {
                    "block2": {
                        "sys:children": [],
                        "sys:flavor": "text",
                        "sys:version": [1.0, 0.0],
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
        map.insert(&mut doc.transact_mut(), "test", "aaa");

        let data = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());

        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(Update::decode_v1(&data).unwrap());

        assert_eq!(doc.transact().store().root_keys(), vec!["test"]);
    }
}
