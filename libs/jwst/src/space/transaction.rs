use crate::utils::JS_INT_RANGE;

use super::*;
use lib0::any::Any;
use yrs::{Map, TransactionMut};

pub struct SpaceTransaction<'a> {
    pub space: &'a Space,
    pub trx: TransactionMut<'a>,
}

impl SpaceTransaction<'_> {
    pub fn remove<S: AsRef<str>>(&mut self, block_id: S) -> bool {
        self.space.remove(&mut self.trx, block_id)
    }

    // create a block with specified flavour
    // if block exists, return the exists block
    pub fn create<B, F>(&mut self, block_id: B, flavour: F) -> JwstResult<Block>
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        self.space.create(&mut self.trx, block_id, flavour)
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) -> JwstResult<()> {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.space.metadata.insert(&mut self.trx, key, bool)?;
            }
            Any::String(text) => {
                self.space
                    .metadata
                    .insert(&mut self.trx, key, text.to_string())?;
            }
            Any::Number(number) => {
                self.space.metadata.insert(&mut self.trx, key, number)?;
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.space
                        .metadata
                        .insert(&mut self.trx, key, number as f64)?;
                } else {
                    self.space.metadata.insert(&mut self.trx, key, number)?;
                }
            }
            Any::Null | Any::Undefined => {
                self.space.metadata.remove(&mut self.trx, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }

        Ok(())
    }

    pub fn commit(&mut self) {
        self.trx.commit();
    }
}
