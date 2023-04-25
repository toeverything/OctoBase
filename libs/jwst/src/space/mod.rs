mod convert;
mod transaction;

use super::{block::MarkdownState, workspace::Pages, *};
use serde::{ser::SerializeMap, Serialize, Serializer};
use std::sync::Arc;
use transaction::SpaceTransaction;
use yrs::{Doc, Map, MapRef, ReadTxn, Transact, TransactionMut, WriteTxn};

//         Workspace
//         /       \
//     Space ... Space
//   /  |  \      /    \
//Block .. Block Block ..Block
pub struct Space {
    workspace_id: String,
    space_id: String,
    doc: Doc,
    pub(super) blocks: MapRef,
    pub(super) updated: MapRef,
    pub(super) metadata: MapRef,
    pages: Pages,
    block_observer_config: Option<Arc<BlockObserverConfig>>,
}

impl Space {
    pub fn new<I, S>(
        trx: &mut TransactionMut,
        doc: Doc,
        pages: Pages,
        workspace_id: I,
        space_id: S,
        block_observer_config: Option<Arc<BlockObserverConfig>>,
    ) -> Self
    where
        I: AsRef<str>,
        S: AsRef<str>,
    {
        let space_id = space_id.as_ref().into();
        let store = trx.store_mut();
        let blocks = doc.get_or_insert_map_with_trx(store, &format!("space:{}", space_id));
        let updated = doc.get_or_insert_map_with_trx(store, constants::space::UPDATED);
        let metadata = doc.get_or_insert_map_with_trx(store, constants::space::META);

        Self {
            workspace_id: workspace_id.as_ref().into(),
            space_id,
            doc,
            blocks,
            updated,
            metadata,
            pages,
            block_observer_config,
        }
    }

    pub fn from_exists<I, S>(
        trx: &TransactionMut,
        doc: Doc,
        workspace_id: I,
        space_id: S,
        block_observer_config: Option<Arc<BlockObserverConfig>>,
    ) -> Option<Self>
    where
        I: AsRef<str>,
        S: AsRef<str>,
    {
        let space_id = space_id.as_ref().into();
        let blocks = trx.get_map(&format!("space:{}", space_id))?;
        let updated = trx.get_map(constants::space::UPDATED)?;
        let metadata = trx.get_map(constants::space::META)?;
        let pages = Pages::new(metadata.get(trx, "pages").and_then(|v| v.to_yarray())?);

        Some(Self {
            workspace_id: workspace_id.as_ref().into(),
            space_id,
            doc,
            blocks,
            updated,
            metadata,
            pages,
            block_observer_config,
        })
    }

    pub fn id(&self) -> String {
        self.workspace_id.clone()
    }

    pub fn space_id(&self) -> String {
        self.space_id.clone()
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client_id()
    }

    pub fn doc(&self) -> Doc {
        self.doc.clone()
    }

    pub fn with_trx<T>(&self, f: impl FnOnce(SpaceTransaction) -> T) -> T {
        let doc = self.doc();
        let trx = SpaceTransaction {
            trx: doc.transact_mut(),
            space: self,
        };

        f(trx)
    }

    pub fn try_with_trx<T>(&self, f: impl FnOnce(SpaceTransaction) -> T) -> Option<T> {
        match self.doc().try_transact_mut() {
            Ok(trx) => {
                let trx = SpaceTransaction { trx, space: self };
                Some(f(trx))
            }
            Err(e) => {
                info!("try_with_trx error: {}", e);
                None
            }
        }
    }

    // get a block if exists
    pub fn get<T, S>(&self, trx: &T, block_id: S) -> Option<Block>
    where
        T: ReadTxn,
        S: AsRef<str>,
    {
        let mut block = Block::from(trx, self, block_id, self.client_id())?;
        if let Some(block_observer_config) = self.block_observer_config.clone() {
            block.subscribe(block_observer_config);
        }
        Some(block)
    }

    pub fn block_count(&self) -> u32 {
        self.blocks.len(&self.doc.transact())
    }

    #[inline]
    pub fn blocks<T, R>(
        &self,
        trx: &T,
        cb: impl FnOnce(Box<dyn Iterator<Item = Block> + '_>) -> R,
    ) -> R
    where
        T: ReadTxn,
    {
        let iterator = self.blocks.iter(trx).map(|(id, block)| {
            Block::from_raw_parts(
                trx,
                self.id(),
                self.space_id(),
                id.to_owned(),
                &self.doc,
                block.to_ymap().unwrap(),
                self.updated.get(trx, id).and_then(|u| u.to_yarray()),
                self.client_id(),
            )
        });

        cb(Box::new(iterator))
    }

    pub fn create<B, F>(
        &self,
        trx: &mut TransactionMut,
        block_id: B,
        flavour: F,
    ) -> JwstResult<Block>
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        info!(
            "create block: {}, flavour: {}",
            block_id.as_ref(),
            flavour.as_ref()
        );
        let mut block = Block::new(trx, self, block_id, flavour, self.client_id())?;
        if let Some(block_observer_config) = self.block_observer_config.clone() {
            block.subscribe(block_observer_config);
        }
        Ok(block)
    }

    pub fn remove<S: AsRef<str>>(&self, trx: &mut TransactionMut, block_id: S) -> bool {
        info!("remove block: {}", block_id.as_ref());
        self.blocks.remove(trx, block_id.as_ref()).is_some()
            && self.updated.remove(trx, block_id.as_ref()).is_some()
    }

    pub fn get_blocks_by_flavour<T>(&self, trx: &T, flavour: &str) -> Vec<Block>
    where
        T: ReadTxn,
    {
        self.blocks(trx, |blocks| {
            blocks
                .filter(|block| block.flavour(trx) == flavour)
                .map(|mut block| {
                    if let Some(block_observer_config) = self.block_observer_config.clone() {
                        block.subscribe(block_observer_config);
                    }
                    block
                })
                .collect::<Vec<_>>()
        })
    }

    /// Check if the block exists in this workspace's blocks.
    pub fn exists<T>(&self, trx: &T, block_id: &str) -> bool
    where
        T: ReadTxn,
    {
        self.blocks.contains_key(trx, block_id.as_ref())
    }

    pub fn shared<T>(&self, trx: &T) -> bool
    where
        T: ReadTxn,
    {
        self.pages.check_shared(trx, &self.space_id)
    }
}

impl Serialize for Space {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let doc = self.doc();
        let trx = doc.transact();
        let mut map = serializer.serialize_map(None)?;
        self.blocks(&trx, |blocks| {
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
            Space::new(
                &mut trx,
                doc.clone(),
                Pages::new(pages),
                "workspace",
                space_id,
                None,
            )
        };
        space.with_trx(|mut t| {
            let block = t.create("test", "text").unwrap();

            block.set(&mut t.trx, "test", "test").unwrap();
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
            let pages = metadata
                .insert(&mut trx, "pages", ArrayPrelim::default())
                .unwrap();
            Space::new(
                &mut trx,
                doc.clone(),
                Pages::new(pages),
                "workspace",
                "space",
                None,
            )
        };

        space.with_trx(|t| {
            assert_eq!(space.id(), "workspace");
            assert_eq!(space.space_id(), "space");
            assert_eq!(space.blocks.len(&t.trx), 0);
            assert_eq!(space.updated.len(&t.trx), 0);
        });

        space.with_trx(|mut t| {
            let block = t.create("block", "text").unwrap();

            assert_eq!(space.blocks.len(&t.trx), 1);
            assert_eq!(space.updated.len(&t.trx), 1);
            assert_eq!(block.block_id(), "block");
            assert_eq!(block.flavour(&t.trx), "text");

            assert_eq!(
                space.get(&t.trx, "block").map(|b| b.block_id()),
                Some("block".to_owned())
            );

            assert!(space.exists(&t.trx, "block"));

            assert!(t.remove("block"));

            assert_eq!(space.blocks.len(&t.trx), 0);
            assert_eq!(space.updated.len(&t.trx), 0);
            assert_eq!(space.get(&t.trx, "block"), None);
            assert!(!space.exists(&t.trx, "block"));
        });

        space.with_trx(|mut t| {
            Block::new(&mut t.trx, &space, "test", "test", 1).unwrap();
            let vec = space.get_blocks_by_flavour(&t.trx, "test");
            assert_eq!(vec.len(), 1);
        });

        let doc = Doc::with_client_id(123);
        let mut trx = doc.transact_mut();
        let metadata = doc.get_or_insert_map_with_trx(trx.store_mut(), constants::space::META);
        let pages = metadata
            .insert(&mut trx, "pages", ArrayPrelim::default())
            .unwrap();
        let space = Space::new(
            &mut trx,
            doc.clone(),
            Pages::new(pages),
            "space",
            "test",
            None,
        );
        assert_eq!(space.client_id(), 123);
    }
}
