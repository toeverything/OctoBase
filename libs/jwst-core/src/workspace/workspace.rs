use std::sync::Arc;

use jwst_codec::{Awareness, Doc, Map, Update};
use serde::{ser::SerializeMap, Serialize, Serializer};
use tokio::sync::RwLock;

use super::*;

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
        doc.apply_update(Update::from_ybinary1(binary)?)?;
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

impl Serialize for Workspace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;

        for space in self.spaces(|spaces| spaces.collect::<Vec<_>>()) {
            map.serialize_entry(&format!("space:{}", space.space_id()), &space)?;
        }

        map.serialize_entry("space:meta", &self.metadata)?;
        map.serialize_entry("space:updated", &self.updated)?;

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
        )
    }
}

#[cfg(test)]
mod test {
    use jwst_codec::StateVector;

    use super::{super::super::Block, *};

    #[test]
    fn doc_load_test() {
        let doc = {
            let mut workspace = Workspace::new("test").unwrap();
            let mut space = workspace.get_space("test").unwrap();

            let mut block = space.create("test", "text").unwrap();
            block.set("test", "test").unwrap();
            workspace.doc()
        };

        let new_doc = {
            let update = doc.encode_state_as_update_v1(&StateVector::default()).unwrap();

            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update).unwrap()).unwrap();
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.get_or_create_map("space:meta").unwrap(),
            new_doc.get_or_create_map("space:meta").unwrap()
        );

        assert_json_diff::assert_json_eq!(
            doc.get_or_create_map("space:updated").unwrap(),
            new_doc.get_or_create_map("space:updated").unwrap(),
        );
    }

    #[test]
    fn workspace() {
        let mut workspace = Workspace::new("test").unwrap();

        {
            assert_eq!(workspace.id(), "test");
            assert_eq!(workspace.updated.len(), 0);
        }

        {
            let mut space = workspace.get_space("test").unwrap();

            let block = space.create("block", "text").unwrap();

            assert_eq!(space.blocks.len(), 1);
            assert_eq!(workspace.updated.len(), 1);
            assert_eq!(block.block_id(), "block");
            assert_eq!(block.flavour(), "text");

            assert_eq!(space.get("block").map(|b| b.block_id()), Some("block".to_owned()));

            assert!(space.exists("block"));

            assert!(space.remove("block"));

            assert_eq!(space.blocks.len(), 0);
            assert_eq!(workspace.updated.len(), 0);
            assert_eq!(space.get("block"), None);
            assert!(!space.exists("block"));
        }

        {
            let mut space = workspace.get_space("test").unwrap();

            Block::new(&mut space, "test", "test", 1).unwrap();
            let vec = space.get_blocks_by_flavour("test");
            assert_eq!(vec.len(), 1);
        }

        let doc = Doc::with_client(123);
        let workspace = Workspace::from_doc(doc, "test").unwrap();
        assert_eq!(workspace.client_id(), 123);
    }

    #[test]
    fn workspace_struct() {
        use assert_json_diff::assert_json_include;

        let mut workspace = Workspace::new("workspace").unwrap();

        {
            let mut space = workspace.get_space("space1").unwrap();
            space.create("block1", "text").unwrap();

            let mut space = workspace.get_space("space2").unwrap();
            space.create("block2", "text").unwrap();
        }

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
        let doc = Doc::default();
        let mut map = doc.get_or_create_map("test").unwrap();
        map.insert("test", "aaa").unwrap();

        let data = doc.encode_state_as_update_v1(&StateVector::default()).unwrap();

        let mut doc = Doc::default();
        doc.apply_update(Update::from_ybinary1(data).unwrap()).unwrap();

        assert_eq!(doc.keys(), vec!["test"]);
    }

    #[test]
    fn test_apply_update_multiple_times() {
        let mut update = {
            let doc = Doc::default();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            {
                let mut space = ws.get_space("space").unwrap();
                let _block = space.create("test", "test1").unwrap();
            }

            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };

        for _ in 0..=2 {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update).unwrap()).unwrap();

            update = doc.encode_state_as_update_v1(&StateVector::default()).unwrap();
        }
    }

    #[test]
    fn test_same_ymap_id_same_source_merge() {
        let update = {
            let doc = Doc::default();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            {
                let mut space = ws.get_space("space").unwrap();
                let _block = space.create("test", "test1").unwrap();
            }

            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };
        let update1 = {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update.clone()).unwrap())
                .unwrap();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            {
                let mut space = ws.get_space("space").unwrap();
                let mut new_block = space.create("test1", "test1").unwrap();
                let mut block = space.get("test").unwrap();
                block.insert_children_at(&mut new_block, 0).unwrap();
            }

            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };
        let update2 = {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update).unwrap()).unwrap();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            {
                let mut space = ws.get_space("space").unwrap();
                let mut new_block = space.create("test2", "test2").unwrap();
                let mut block = space.get("test").unwrap();
                block.insert_children_at(&mut new_block, 0).unwrap();
            }

            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };

        {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update1.clone()).unwrap())
                .unwrap();
            doc.apply_update(Update::from_ybinary1(update2.clone()).unwrap())
                .unwrap();

            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            let block = {
                let space = ws.get_space("space").unwrap();
                space.get("test").unwrap()
            };
            println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

            {
                let space = ws.get_space("space").unwrap();
                let block = space.get("test").unwrap();
                let mut children = block.children();
                children.sort();
                assert_eq!(children, vec!["test1".to_owned(), "test2".to_owned()]);
            }
        }

        // TODO: fix merge update
        // {
        //     let merged_update = merge_updates_v1(&[&update1,
        // &update2]).unwrap();     let mut doc = Doc::default();
        //     doc.apply_update(
        //         Update::from_ybinary1(merged_update.into_ybinary1().
        // unwrap()).unwrap(),     )
        //     .unwrap();

        //     let mut ws = Workspace::from_doc(doc, "test").unwrap();
        //     let block = {
        //         let space = ws.get_space("space").unwrap();
        //         space.get("test").unwrap()
        //     };
        //     println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

        //     {
        //         let space = ws.get_space("space").unwrap();
        //         let block = space.get("test").unwrap();
        //         let mut children = block.children();
        //         children.sort();
        //         assert_eq!(children, vec!["test1".to_owned(),
        // "test2".to_owned()]);         //
        // assert_eq!(block.get("test1").unwrap().to_string(), "test1");
        //         // assert_eq!(block.get("test2").unwrap().to_string(),
        // "test2");     }
        // }
    }

    #[test]
    fn test_same_ymap_id_different_source_merge() {
        let update = {
            let doc = Doc::default();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            ws.get_space("space").unwrap();
            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };

        let update1 = {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update.clone()).unwrap())
                .unwrap();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            {
                let mut space = ws.get_space("space").unwrap();
                let mut new_block = space.create("test1", "test1").unwrap();
                let mut block = space.create("test", "test1").unwrap();
                block.insert_children_at(&mut new_block, 0).unwrap();
            }

            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };
        let update2 = {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update).unwrap()).unwrap();
            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            {
                let mut space = ws.get_space("space").unwrap();
                let mut _new_block = space.create("test2", "test2").unwrap();
                // TODO: fix merge update crash if we create same block in two
                // doc then merge them let mut block =
                // space.create("test", "test1").unwrap();
                // block.insert_children_at(&mut new_block, 0).unwrap();
            }

            ws.doc().encode_state_as_update_v1(&StateVector::default()).unwrap()
        };

        {
            let mut doc = Doc::default();
            doc.apply_update(Update::from_ybinary1(update1.clone()).unwrap())
                .unwrap();
            doc.apply_update(Update::from_ybinary1(update2.clone()).unwrap())
                .unwrap();

            let mut ws = Workspace::from_doc(doc, "test").unwrap();
            let block = {
                let space = ws.get_space("space").unwrap();
                space.get("test").unwrap()
            };
            println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

            {
                let space = ws.get_space("space").unwrap();
                let block = space.get("test").unwrap();
                let mut children = block.children();
                children.sort();
                assert_ne!(children, vec!["test1".to_owned(), "test2".to_owned()]);
            }
        }

        // TODO: fix merge update
        // {
        //     let merged_update = merge_updates_v1(&[&update1,
        // &update2]).unwrap();     let mut doc = Doc::default();
        //     doc.apply_update(
        //         Update::from_ybinary1(merged_update.into_ybinary1().
        // unwrap()).unwrap(),     )
        //     .unwrap();

        //     let mut ws = Workspace::from_doc(doc, "test").unwrap();
        //     let block = {
        //         let space = ws.get_space("space").unwrap();
        //         space.get("test").unwrap()
        //     };
        //     println!("{:?}", serde_json::to_string_pretty(&block).unwrap());

        //     {
        //         let space = ws.get_space("space").unwrap();
        //         let block = space.get("test").unwrap();
        //         let mut children = block.children();
        //         children.sort();
        //         assert_ne!(children, vec!["test1".to_owned(),
        // "test2".to_owned()]);         // assert_eq!(block.get(
        // "test1").unwrap().to_string(), "test1");         //
        // assert_eq!(block.get( "test2").unwrap().to_string(), "test2");
        //     }
        // }
    }
}
