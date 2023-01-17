use crate::utils::JS_INT_RANGE;

use super::*;
use lib0::any::Any;
use yrs::Transaction;

pub struct WorkspaceTransaction<'a> {
    pub ws: &'a Workspace,
    pub trx: Transaction,
}

unsafe impl Send for WorkspaceTransaction<'_> {}

impl WorkspaceTransaction<'_> {
    pub fn remove<S: AsRef<str>>(&mut self, block_id: S) -> bool {
        info!("remove block: {}", block_id.as_ref());
        self.ws
            .content
            .blocks
            .remove(&mut self.trx, block_id.as_ref())
            .is_some()
            && self
                .ws
                .updated()
                .remove(&mut self.trx, block_id.as_ref())
                .is_some()
    }

    // create a block with specified flavor
    // if block exists, return the exists block
    pub fn create<B, F>(&self, block_id: B, flavor: F) -> Block
    where
        B: AsRef<str>,
        F: AsRef<str>,
    {
        info!("create block: {}", block_id.as_ref());
        Block::new(&self.ws, block_id, flavor, self.ws.client_id())
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.ws.metadata().insert(&mut self.trx, key, bool);
            }
            Any::String(text) => {
                self.ws
                    .metadata()
                    .insert(&mut self.trx, key, text.to_string());
            }
            Any::Number(number) => {
                self.ws.metadata().insert(&mut self.trx, key, number);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.ws.metadata().insert(&mut self.trx, key, number as f64);
                } else {
                    self.ws.metadata().insert(&mut self.trx, key, number);
                }
            }
            Any::Null | Any::Undefined => {
                self.ws.metadata().remove(&mut self.trx, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }
    }

    pub fn commit(&mut self) {
        self.trx.commit();
    }
}
