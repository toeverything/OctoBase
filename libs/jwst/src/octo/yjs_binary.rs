//! Features related to storing and exporting yjs compatible diffs and updates.

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::errors;

pub struct OctoWorkspaceMigrator<'doc> {
    pub(crate) yrs_txn_mut: yrs::TransactionMut<'doc>,
}

pub struct OctoWorkspaceExporter<'doc> {
    pub(crate) yrs_txn: yrs::Transaction<'doc>,
}

/// Hmmm... too limited?
impl<'doc> OctoWorkspaceExporter<'doc> {
    pub fn encode_state_as_update_v1_from_empty(&self) -> Vec<u8> {
        use yrs::ReadTxn;
        self.yrs_txn.encode_state_as_update_v1(&Default::default())
    }
}

impl<'doc> OctoWorkspaceMigrator<'doc> {
    pub fn apply_binary_update_v1(
        &mut self,
        update: &[u8],
    ) -> Result<(), errors::OctoWorkspaceUpdateError> {
        use yrs::updates::decoder::Decode;
        match yrs::Update::decode_v1(update) {
            Ok(update) => {
                match catch_unwind(AssertUnwindSafe(|| self.yrs_txn_mut.apply_update(update))) {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        log::warn!("update merge failed, skip it: {:?}", err);
                        Err(errors::OctoWorkspaceUpdateError::FailedToApplyUpdate { panicked: err })
                    }
                }
            }
            Err(err) => {
                log::info!("failed to decode update: {:?}", err);
                Err(errors::OctoWorkspaceUpdateError::FailedToParseBinary {
                    source: Box::new(err),
                })
            }
        }
    }

    /// It is not possible to cancel a transaction AFAIK.
    ///
    /// See [y-crdt#236: "`impl fn abort(self)` on `TransactionMut`"](https://github.com/y-crdt/y-crdt/issues/236)
    pub fn commit(mut self) {
        // commit would happen on drop any ways, but we'll be explicit, here.
        self.yrs_txn_mut.commit();
    }
}
