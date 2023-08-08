mod convert;

use super::{block::MarkdownState, workspace::Pages, *};
use jwst_codec::{Any, Doc, Map};
use serde::{ser::SerializeMap, Serialize, Serializer};

//         Workspace
//         /       \
//     Space ... Space
//   /  |  \      /    \
//Block .. Block Block ..Block
pub struct Space {
    workspace_id: String,
    space_id: String,
    doc: Doc,
    pub(super) blocks: Map,
    pub(super) updated: Map,
    pub(super) metadata: Map,
    pages: Pages,
}

impl Space {
    pub fn new<I, S>(doc: Doc, pages: Pages, workspace_id: I, space_id: S) -> JwstResult<Self>
    where
        I: AsRef<str>,
        S: AsRef<str>,
    {
        let space_id = space_id.as_ref().into();
        let blocks = doc.get_or_create_map(&format!("space:{}", space_id))?;
        let updated = doc.get_or_create_map(constants::space::UPDATED)?;
        let metadata = doc.get_or_create_map(constants::space::META)?;

        Ok(Self {
            workspace_id: workspace_id.as_ref().into(),
            space_id,
            doc,
            blocks,
            updated,
            metadata,
            pages,
        })
    }

    pub fn from_exists<I, S>(doc: Doc, workspace_id: I, space_id: S) -> Option<Self>
    where
        I: AsRef<str>,
        S: AsRef<str>,
    {
        let space_id = space_id.as_ref().into();
        let blocks = doc.get_map(&format!("space:{}", space_id)).ok()?;
        let updated = doc.get_map(constants::space::UPDATED).ok()?;
        let metadata = doc.get_map(constants::space::META).ok()?;
        let pages = Pages::new(metadata.get("pages").and_then(|v| v.to_array())?);

        Some(Self {
            workspace_id: workspace_id.as_ref().into(),
            space_id,
            doc,
            blocks,
            updated,
            metadata,
            pages,
        })
    }

    pub fn id(&self) -> String {
        self.workspace_id.clone()
    }

    pub fn space_id(&self) -> String {
        self.space_id.clone()
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client()
    }

    pub fn doc(&self) -> Doc {
        self.doc.clone()
    }

    // get a block if exists
    pub fn get<S>(&self, block_id: S) -> Option<Block>
    where
        S: AsRef<str>,
    {
        Block::from(self, block_id, self.client_id())
    }

    pub fn block_count(&self) -> u64 {
        self.blocks.len()
    }

    #[inline]
    pub fn blocks<R>(&self, cb: impl FnOnce(Box<dyn Iterator<Item = Block> + '_>) -> R) -> R {
        let iterator = self.blocks.iter().map(|(id, block)| {
            Block::from_raw_parts(
                self.id(),
                self.space_id(),
                id.to_owned(),
                &self.doc,
                block.to_map().unwrap(),
                self.updated.get(id).and_then(|u| u.to_array()),
                self.client_id(),
            )
        });

        cb(Box::new(iterator))
    }

    pub fn create<B, F>(&self, block_id: B, flavour: F) -> JwstResult<Block>
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        info!(
            "create block: {}, flavour: {}",
            block_id.as_ref(),
            flavour.as_ref()
        );
        Block::new(self, block_id, flavour, self.client_id())
    }

    pub fn remove<S: AsRef<str>>(&self, block_id: S) -> bool {
        info!("remove block: {}", block_id.as_ref());
        self.blocks.remove(block_id.as_ref()) && self.updated.remove(block_id.as_ref())
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) -> JwstResult {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Null | Any::Undefined => {
                self.metadata.remove(&key);
            }
            value => {
                self.metadata.insert(key, value);
            }
        }

        Ok(())
    }

    pub fn get_blocks_by_flavour(&self, flavour: &str) -> Vec<Block> {
        self.blocks(|blocks| {
            blocks
                .filter(|block| block.flavour() == flavour)
                .collect::<Vec<_>>()
        })
    }

    /// Check if the block exists in this workspace's blocks.
    pub fn exists(&self, block_id: &str) -> bool {
        self.blocks.contains_key(block_id)
    }

    pub fn shared(&self) -> bool {
        self.pages.check_shared(&self.space_id)
    }
}

impl Serialize for Space {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        self.blocks(|blocks| {
            let blocks = blocks.collect::<Vec<_>>();
            for block in blocks {
                map.serialize_entry(&block.block_id(), &block)?;
            }
            Ok(())
        })?;

        map.end()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tracing::info;
    use yrs::{types::ToJson, updates::decoder::Decode, ArrayPrelim, Doc, StateVector, Update};

    #[test]
    fn doc_load_test() {
        let space_id = "space";
        let space_string = format!("space:{}", space_id);

        let doc = Doc::new();

        let space = {
            let mut trx = doc.transact_mut();
            let metadata = doc.get_or_insert_map_with_trx(trx.store_mut(), constants::space::META);
            let pages = metadata
                .insert(&mut trx, "pages", ArrayPrelim::default())
                .unwrap();
            Space::new(doc.clone(), Pages::new(pages), "workspace", space_id)
        };
        space.with_trx(|mut t| {
            let block = t.create("test", "text").unwrap();

            block.set("test", "test").unwrap();
        });

        let doc = space.doc();

        let new_doc = {
            let update = doc
                .transact()
                .encode_state_as_update_v1(&StateVector::default())
                .and_then(|update| Update::decode_v1(&update));
            let doc = Doc::default();
            {
                let mut trx = doc.transact_mut();
                match update {
                    Ok(update) => trx.apply_update(update),
                    Err(err) => info!("failed to decode update: {:?}", err),
                }
                trx.commit();
            }
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.get_or_insert_map(&space_string)
                .to_json(&doc.transact()),
            new_doc
                .get_or_insert_map(&space_string)
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
    fn space() {
        let doc = Doc::new();
        let space = {
            let mut trx = doc.transact_mut();
            let metadata = doc.get_or_insert_map_with_trx(trx.store_mut(), constants::space::META);
            let pages = metadata.insert("pages", ArrayPrelim::default()).unwrap();
            Space::new(doc.clone(), Pages::new(pages), "workspace", "space").unwrap()
        };

        space.with_trx(|t| {
            assert_eq!(space.id(), "workspace");
            assert_eq!(space.space_id(), "space");
            assert_eq!(space.blocks.len(), 0);
            assert_eq!(space.updated.len(), 0);
        });

        space.with_trx(|mut t| {
            let block = t.create("block", "text").unwrap();

            assert_eq!(space.blocks.len(), 1);
            assert_eq!(space.updated.len(), 1);
            assert_eq!(block.block_id(), "block");
            assert_eq!(block.flavour(), "text");

            assert_eq!(
                space.get("block").map(|b| b.block_id()),
                Some("block".to_owned())
            );

            assert!(space.exists("block"));

            assert!(t.remove("block"));

            assert_eq!(space.blocks.len(), 0);
            assert_eq!(space.updated.len(), 0);
            assert_eq!(space.get("block"), None);
            assert!(!space.exists("block"));
        });

        space.with_trx(|mut t| {
            Block::new(&space, "test", "test", 1).unwrap();
            let vec = space.get_blocks_by_flavour("test");
            assert_eq!(vec.len(), 1);
        });

        let doc = Doc::with_client_id(123);
        let mut trx = doc.transact_mut();
        let metadata = doc.get_or_insert_map_with_trx(trx.store_mut(), constants::space::META);
        let pages = metadata
            .insert(&mut trx, "pages", ArrayPrelim::default())
            .unwrap();
        let space = Space::new(doc.clone(), Pages::new(pages), "space", "test");
        assert_eq!(space.client_id(), 123);
    }
}
