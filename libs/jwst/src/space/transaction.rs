use crate::utils::JS_INT_RANGE;

use super::*;
use lib0::any::Any;
use yrs::{Map, TransactionMut};

pub struct SpaceTransaction<'a> {
    pub space: &'a Space,
    pub trx: TransactionMut<'a>,
}

unsafe impl Send for SpaceTransaction<'_> {}

impl SpaceTransaction<'_> {
    pub fn remove<S: AsRef<str>>(&mut self, block_id: S) -> bool {
        info!("remove block: {}", block_id.as_ref());
        self.space
            .blocks
            .remove(&mut self.trx, block_id.as_ref())
            .is_some()
            && self
                .space
                .updated
                .remove(&mut self.trx, block_id.as_ref())
                .is_some()
    }

    // create a block with specified flavor
    // if block exists, return the exists block
    pub fn create<B, F>(&mut self, block_id: B, flavor: F) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        info!(
            "create block: {}, flavour: {}",
            block_id.as_ref(),
            flavor.as_ref()
        );
        Block::new(
            &mut self.trx,
            self.space,
            block_id,
            flavor,
            self.space.client_id(),
        )
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.space.metadata.insert(&mut self.trx, key, bool);
            }
            Any::String(text) => {
                self.space
                    .metadata
                    .insert(&mut self.trx, key, text.to_string());
            }
            Any::Number(number) => {
                self.space.metadata.insert(&mut self.trx, key, number);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.space
                        .metadata
                        .insert(&mut self.trx, key, number as f64);
                } else {
                    self.space.metadata.insert(&mut self.trx, key, number);
                }
            }
            Any::Null | Any::Undefined => {
                self.space.metadata.remove(&mut self.trx, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }
    }

    pub fn commit(&mut self) {
        self.trx.commit();
    }
}
