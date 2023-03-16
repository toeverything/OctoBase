mod transaction;

use super::*;
use transaction::SpaceTransaction;
use yrs::{Doc, Map, MapRef, ReadTxn, Transact, TransactionMut, WriteTxn};

pub struct Space {
    id: String,
    space_id: String,
    doc: Doc,
    pub(super) blocks: MapRef,
    pub(super) updated: MapRef,
    pub(super) metadata: MapRef,
}

impl Space {
    pub fn new<I, S>(trx: &mut TransactionMut, doc: Doc, id: I, space_id: S) -> Self
    where
        I: AsRef<str>,
        S: AsRef<str>,
    {
        let space_id = space_id.as_ref().into();
        let mut store = trx.store_mut();
        let blocks = doc.get_or_insert_map_with_trx(&mut store, &format!("space:{}", space_id));
        let updated = doc.get_or_insert_map_with_trx(&mut store, constants::space::UPDATED);
        let metadata = doc.get_or_insert_map_with_trx(&mut store, constants::space::META);

        Self {
            id: id.as_ref().into(),
            space_id,
            doc,
            blocks,
            updated,
            metadata,
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
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
        Block::from(trx, self, block_id, self.client_id())
    }

    pub fn block_count(&self) -> u32 {
        self.blocks.len(&self.doc.transact())
    }

    #[inline]
    pub fn blocks<T, R>(&self, trx: &T, cb: impl Fn(Box<dyn Iterator<Item = Block> + '_>) -> R) -> R
    where
        T: ReadTxn,
    {
        let iterator =
            self.blocks
                .iter(trx)
                .zip(self.updated.iter(trx))
                .map(|((id, block), (_, updated))| {
                    Block::from_raw_parts(
                        trx,
                        id.to_owned(),
                        &self.doc,
                        block.to_ymap().unwrap(),
                        updated.to_yarray().unwrap(),
                        self.client_id(),
                    )
                });

        cb(Box::new(iterator))
    }

    pub fn create<B, F>(&self, trx: &mut TransactionMut, block_id: B, flavor: F) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        info!(
            "create block: {}, flavour: {}",
            block_id.as_ref(),
            flavor.as_ref()
        );
        Block::new(trx, self, block_id, flavor, self.client_id())
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
                .filter(|block| block.flavor(trx) == flavour)
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
}

#[cfg(test)]
mod test {
    use super::*;
    use tracing::info;
    use yrs::{types::ToJson, updates::decoder::Decode, Doc, StateVector, Update};

    #[test]
    fn doc_load_test() {
        let space_id = "space";
        let space_string = format!("space:{}", space_id);

        let doc = Doc::new();

        let space = {
            let mut trx = doc.transact_mut();
            Space::new(&mut trx, doc.clone(), "workspace", space_id)
        };
        space.with_trx(|mut t| {
            let block = t.create("test", "text");

            block.set(&mut t.trx, "test", "test");
        });

        let doc = space.doc();

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
            Space::new(&mut trx, doc.clone(), "workspace", "space")
        };

        space.with_trx(|t| {
            assert_eq!(space.id(), "workspace");
            assert_eq!(space.space_id(), "space");
            assert_eq!(space.blocks.len(&t.trx), 0);
            assert_eq!(space.updated.len(&t.trx), 0);
        });

        space.with_trx(|mut t| {
            let block = t.create("block", "text");

            assert_eq!(space.blocks.len(&t.trx), 1);
            assert_eq!(space.updated.len(&t.trx), 1);
            assert_eq!(block.id(), "block");
            assert_eq!(block.flavor(&t.trx), "text");

            assert_eq!(
                space.get(&t.trx, "block").map(|b| b.id()),
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
            Block::new(&mut t.trx, &space, "test", "test", 1);
            let vec = space.get_blocks_by_flavour(&t.trx, "test");
            assert_eq!(vec.len(), 1);
        });

        let doc = Doc::with_client_id(123);
        let mut trx = doc.transact_mut();
        let space = Space::new(&mut trx, doc.clone(), "space", "test");
        assert_eq!(space.client_id(), 123);
    }
}
