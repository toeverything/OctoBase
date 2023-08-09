use super::*;
use jwst_codec::{Awareness, Doc, Map, Update};
use serde::{ser::SerializeMap, Serialize, Serializer};
use std::sync::Arc;
use tokio::sync::RwLock;
use yrs::{types::map::MapEvent, Subscription, TransactionMut};

pub type MapSubscription = Subscription<Arc<dyn Fn(&TransactionMut, &MapEvent)>>;

pub struct Workspace {
    workspace_id: String,
    pub(super) awareness: Arc<RwLock<Awareness>>,
    pub(super) doc: Doc,
    pub(crate) updated: Map,
    pub(crate) metadata: Map,
}

unsafe impl Send for Workspace {}
unsafe impl Sync for Workspace {}

impl Workspace {
    pub fn new<S: AsRef<str>>(id: S) -> JwstResult<Self> {
        let doc = Doc::default();
        Self::from_doc(doc, id)
    }

    pub fn from_binary(binary: Vec<u8>, workspace_id: &str) -> JwstResult<Self> {
        let mut doc = Doc::default();
        doc.apply_update(Update::from_ybinary1(binary)?);
        Self::from_doc(doc, workspace_id)
    }

    pub fn from_doc<S: AsRef<str>>(doc: Doc, workspace_id: S) -> JwstResult<Self> {
        let updated = doc.get_or_create_map("space:updated")?;
        let metadata = doc.get_or_create_map("space:meta")?;

        Ok(Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness: Arc::new(RwLock::new(Awareness::new(doc.client()))),
            doc,
            updated,
            metadata,
        })
    }

    fn from_raw<S: AsRef<str>>(
        workspace_id: S,
        awareness: Arc<RwLock<Awareness>>,
        doc: Doc,
        updated: Map,
        metadata: Map,
    ) -> Workspace {
        Self {
            workspace_id: workspace_id.as_ref().to_string(),
            awareness,
            doc,
            updated,
            metadata,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.updated.is_empty()
    }

    pub fn id(&self) -> String {
        self.workspace_id.clone()
    }

    pub fn doc_guid(&self) -> &str {
        self.doc.guid()
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client()
    }

    pub fn doc(&self) -> Doc {
        self.doc.clone()
    }
}

// TODO: Implement Serialize for Workspace
// impl Serialize for Workspace {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let mut map = serializer.serialize_map(None)?;

//         for space in self.spaces(|spaces| spaces.collect::<Vec<_>>()) {
//             map.serialize_entry(&format!("space:{}", space.space_id()), &space)?;
//         }

//         map.serialize_entry("space:meta", &self.metadata)?;
//         map.serialize_entry("space:updated", &self.updated)?;

//         map.end()
//     }
// }

impl Clone for Workspace {
    fn clone(&self) -> Self {
        Self::from_raw(
            &self.workspace_id,
            self.awareness.clone(),
            self.doc.clone(),
            self.updated.clone(),
            self.metadata.clone(),
        )
    }
}

// #[cfg(test)]
// mod test {
//     use super::{super::super::Block, *};
//     use tracing::info;
//     use yrs::{updates::decoder::Decode, Doc, Map, ReadTxn, StateVector, Update};

//     #[test]
//     fn doc_load_test() {
//         let workspace = Workspace::new("test").unwrap();
//         workspace.with_trx(|mut t| {
//             let space = t.get_space("test");

//             let block = space.create("test", "text").unwrap();

//             block.set("test", "test").unwrap();
//         });

//         let doc = workspace.doc();

//         let new_doc = {
//             let update = doc
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default());
//             let doc = Doc::default();
//             {
//                 let mut trx = doc.transact_mut();
//                 match update.and_then(|update| Update::decode_v1(&update)) {
//                     Ok(update) => trx.apply_update(update),
//                     Err(err) => info!("failed to decode update: {:?}", err),
//                 }
//                 trx.commit();
//             }
//             doc
//         };

//         assert_json_diff::assert_json_eq!(
//             doc.get_or_insert_map("space:meta").to_json(&doc.transact()),
//             new_doc
//                 .get_or_insert_map("space:meta")
//                 .to_json(&doc.transact())
//         );

//         assert_json_diff::assert_json_eq!(
//             doc.get_or_insert_map("space:updated")
//                 .to_json(&doc.transact()),
//             new_doc
//                 .get_or_insert_map("space:updated")
//                 .to_json(&doc.transact())
//         );
//     }

//     #[test]
//     fn workspace() {
//         let workspace = Workspace::new("test");

//         workspace.with_trx(|t| {
//             assert_eq!(workspace.id(), "test");
//             assert_eq!(workspace.updated.len(&t.trx), 0);
//         });

//         workspace.with_trx(|mut t| {
//             let space = t.get_space("test");

//             let block = space.create("block", "text").unwrap();

//             assert_eq!(space.blocks.len(&t.trx), 1);
//             assert_eq!(workspace.updated.len(&t.trx), 1);
//             assert_eq!(block.block_id(), "block");
//             assert_eq!(block.flavour(&t.trx), "text");

//             assert_eq!(
//                 space.get(&t.trx, "block").map(|b| b.block_id()),
//                 Some("block".to_owned())
//             );

//             assert!(space.exists(&t.trx, "block"));

//             assert!(space.remove("block"));

//             assert_eq!(space.blocks.len(&t.trx), 0);
//             assert_eq!(workspace.updated.len(&t.trx), 0);
//             assert_eq!(space.get(&t.trx, "block"), None);
//             assert!(!space.exists(&t.trx, "block"));
//         });

//         workspace.with_trx(|mut t| {
//             let space = t.get_space("test");

//             Block::new(&space, "test", "test", 1).unwrap();
//             let vec = space.get_blocks_by_flavour("test");
//             assert_eq!(vec.len(), 1);
//         });

//         let doc = Doc::with_client_id(123);
//         let workspace = Workspace::from_doc(doc, "test");
//         assert_eq!(workspace.client_id(), 123);
//     }

//     #[test]
//     fn workspace_struct() {
//         use assert_json_diff::assert_json_include;

//         let workspace = Workspace::new("workspace");

//         workspace.with_trx(|mut t| {
//             let space = t.get_space("space1");
//             space.create("block1", "text").unwrap();

//             let space = t.get_space("space2");
//             space.create("block2", "text").unwrap();
//         });

//         assert_json_include!(
//             actual: serde_json::to_value(&workspace).unwrap(),
//             expected: serde_json::json!({
//                 "space:space1": {
//                     "block1": {
//                         "sys:children": [],
//                         "sys:flavour": "text",
//                     }
//                 },
//                 "space:space2": {
//                     "block2": {
//                         "sys:children": [],
//                         "sys:flavour": "text",
//                     }
//                 },
//                 "space:updated": {
//                     "block1": [[]],
//                     "block2": [[]],
//                 },
//                 "space:meta": {}
//             })
//         );
//     }

//     #[test]
//     fn scan_doc() {
//         let doc = Doc::new();
//         let map = doc.get_or_insert_map("test");
//         map.insert(&mut doc.transact_mut(), "test", "aaa").unwrap();

//         let data = doc
//             .transact()
//             .encode_state_as_update_v1(&StateVector::default())
//             .unwrap();

//         let doc = Doc::new();
//         doc.transact_mut()
//             .apply_update(Update::decode_v1(&data).unwrap());

//         assert_eq!(doc.transact().store().root_keys(), vec!["test"]);
//     }

//     #[test]
//     fn test_same_ymap_id_same_source_merge() {
//         let update = {
//             let doc = Doc::new();
//             let ws = Workspace::from_doc(doc, "test");
//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let _block = space.create("test", "test1").unwrap();
//             });

//             ws.doc()
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default())
//                 .unwrap()
//         };
//         let update1 = {
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update).unwrap());
//             let ws = Workspace::from_doc(doc, "test");
//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let new_block = space.create("test1", "test1").unwrap();
//                 let block = space.get("test").unwrap();
//                 block.insert_children_at(&new_block, 0).unwrap();
//             });

//             ws.doc()
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default())
//                 .unwrap()
//         };
//         let update2 = {
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update).unwrap());
//             let ws = Workspace::from_doc(doc, "test");
//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let new_block = space.create("test2", "test2").unwrap();
//                 let block = space.get("test").unwrap();
//                 block.insert_children_at(&new_block, 0).unwrap();
//             });

//             ws.doc()
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default())
//                 .unwrap()
//         };

//         {
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update1).unwrap());
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update2).unwrap());

//             let ws = Workspace::from_doc(doc, "test");
//             let block = ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 space.get(&t.trx, "test").unwrap()
//             });
//             println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let block = space.get(&t.trx, "test").unwrap();
//                 let mut children = block.children(&t.trx);
//                 children.sort();
//                 assert_eq!(children, vec!["test1".to_owned(), "test2".to_owned()]);
//             });
//         }
//         {
//             let merged_update = yrs::merge_updates_v1(&[&update1, &update2]).unwrap();
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&merged_update).unwrap());

//             let ws = Workspace::from_doc(doc, "test");
//             let block = ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 space.get(&t.trx, "test").unwrap()
//             });
//             println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let block = space.get(&t.trx, "test").unwrap();
//                 let mut children = block.children(&t.trx);
//                 children.sort();
//                 assert_eq!(children, vec!["test1".to_owned(), "test2".to_owned()]);
//                 // assert_eq!(block.get(&t.trx, "test1").unwrap().to_string(), "test1");
//                 // assert_eq!(block.get(&t.trx, "test2").unwrap().to_string(), "test2");
//             });
//         }
//     }

//     #[test]
//     fn test_same_ymap_id_different_source_merge() {
//         let update = {
//             let doc = Doc::new();
//             let ws = Workspace::from_doc(doc, "test");
//             ws.with_trx(|mut t| {
//                 t.get_space("space");
//             });

//             ws.doc()
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default())
//                 .unwrap()
//         };
//         let update1 = {
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update).unwrap());
//             let ws = Workspace::from_doc(doc, "test");
//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let new_block = space.create("test1", "test1").unwrap();
//                 let block = space.create("test", "test1").unwrap();
//                 block.insert_children_at(&new_block, 0).unwrap();
//             });

//             ws.doc()
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default())
//                 .unwrap()
//         };
//         let update2 = {
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update).unwrap());
//             let ws = Workspace::from_doc(doc, "test");
//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let new_block = space.create("test2", "test2").unwrap();
//                 let block = space.create("test", "test1").unwrap();
//                 block.insert_children_at(&new_block, 0).unwrap();
//             });

//             ws.doc()
//                 .transact()
//                 .encode_state_as_update_v1(&StateVector::default())
//                 .unwrap()
//         };

//         {
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update1).unwrap());
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&update2).unwrap());

//             let ws = Workspace::from_doc(doc, "test");
//             let block = ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 space.get(&t.trx, "test").unwrap()
//             });
//             println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let block = space.get(&t.trx, "test").unwrap();
//                 let mut children = block.children(&t.trx);
//                 children.sort();
//                 assert_ne!(children, vec!["test1".to_owned(), "test2".to_owned()]);
//             });
//         }
//         {
//             let merged_update = yrs::merge_updates_v1(&[&update1, &update2]).unwrap();
//             let doc = Doc::new();
//             doc.transact_mut()
//                 .apply_update(Update::decode_v1(&merged_update).unwrap());

//             let ws = Workspace::from_doc(doc, "test");
//             let block = ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 space.get(&t.trx, "test").unwrap()
//             });
//             println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

//             ws.with_trx(|mut t| {
//                 let space = t.get_space("space");
//                 let block = space.get(&t.trx, "test").unwrap();
//                 let mut children = block.children(&t.trx);
//                 children.sort();
//                 assert_ne!(children, vec!["test1".to_owned(), "test2".to_owned()]);
//                 // assert_eq!(block.get(&t.trx, "test1").unwrap().to_string(), "test1");
//                 // assert_eq!(block.get(&t.trx, "test2").unwrap().to_string(), "test2");
//             });
//         }
//     }
// }
