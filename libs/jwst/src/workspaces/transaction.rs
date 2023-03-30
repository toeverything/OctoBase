use crate::utils::JS_INT_RANGE;

use super::*;
use lib0::any::Any;
use yrs::{Map, ReadTxn, TransactionMut};

pub struct WorkspaceTransaction<'a> {
    pub ws: &'a Workspace,
    pub trx: TransactionMut<'a>,
}

unsafe impl Send for WorkspaceTransaction<'_> {}

const RESERVE_SPACE: [&str; 2] = [constants::space::META, constants::space::UPDATED];

impl WorkspaceTransaction<'_> {
    pub fn get_space<S: AsRef<str>>(&mut self, space_id: S) -> Space {
        Space::new(&mut self.trx, self.ws.doc(), self.ws.id(), space_id)
    }

    pub fn get_exists_space<S: AsRef<str>>(&self, space_id: S) -> Option<Space> {
        Space::from_exists(&self.trx, self.ws.doc(), self.ws.id(), space_id)
    }

    /// The compatibility interface for keck/jni/swift, this api was outdated.
    pub fn get_blocks(&mut self) -> Space {
        self.get_space("blocks")
    }

    #[inline]
    pub fn spaces<R>(&self, cb: impl FnOnce(Box<dyn Iterator<Item = Space> + '_>) -> R) -> R {
        let keys = self.trx.store().root_keys();
        let iterator = keys.iter().filter_map(|key| {
            if key.starts_with("space:") && !RESERVE_SPACE.contains(&key.as_str()) {
                Space::from_exists(&self.trx, self.ws.doc(), self.ws.id(), &key[6..])
            } else {
                None
            }
        });

        cb(Box::new(iterator))
    }

    pub fn set_metadata(&mut self, key: &str, value: impl Into<Any>) -> JwstResult<()> {
        info!("set metadata: {}", key);
        let key = key.to_string();
        match value.into() {
            Any::Bool(bool) => {
                self.ws.metadata.insert(&mut self.trx, key, bool)?;
            }
            Any::String(text) => {
                self.ws
                    .metadata
                    .insert(&mut self.trx, key, text.to_string())?;
            }
            Any::Number(number) => {
                self.ws.metadata.insert(&mut self.trx, key, number)?;
            }
            Any::BigInt(number) => {
                if JS_INT_RANGE.contains(&number) {
                    self.ws.metadata.insert(&mut self.trx, key, number as f64)?;
                } else {
                    self.ws.metadata.insert(&mut self.trx, key, number)?;
                }
            }
            Any::Null | Any::Undefined => {
                self.ws.metadata.remove(&mut self.trx, &key);
            }
            Any::Buffer(_) | Any::Array(_) | Any::Map(_) => {}
        }

        Ok(())
    }

    pub fn commit(&mut self) {
        self.trx.commit();
    }
}
