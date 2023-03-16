use crate::utils::JS_INT_RANGE;

use super::*;
use lib0::any::Any;
use yrs::{Map, TransactionMut};

pub struct WorkspaceTransaction<'a> {
    pub ws: &'a Workspace,
    pub trx: TransactionMut<'a>,
}

unsafe impl Send for WorkspaceTransaction<'_> {}

impl WorkspaceTransaction<'_> {
    pub fn get_space<S: AsRef<str>>(&mut self, space_id: S) -> Space {
        Space::new(&mut self.trx, self.ws.doc(), self.ws.id(), space_id)
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.ws.metadata.insert(&mut self.trx, key, bool);
            }
            Any::String(text) => {
                self.ws
                    .metadata
                    .insert(&mut self.trx, key, text.to_string());
            }
            Any::Number(number) => {
                self.ws.metadata.insert(&mut self.trx, key, number);
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.ws.metadata.insert(&mut self.trx, key, number as f64);
                } else {
                    self.ws.metadata.insert(&mut self.trx, key, number);
                }
            }
            Any::Null | Any::Undefined => {
                self.ws.metadata.remove(&mut self.trx, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }
    }

    pub fn commit(&mut self) {
        self.trx.commit();
    }
}
