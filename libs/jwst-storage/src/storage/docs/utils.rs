use super::{entities::prelude::*, types::JwstStorageResult, *};
use std::panic::{catch_unwind, AssertUnwindSafe};
use yrs::{updates::decoder::Decode, Doc, ReadTxn, StateVector, Transact, Update};

// apply all updates to the given doc
pub fn migrate_update(
    update_records: Vec<<Docs as EntityTrait>::Model>,
    doc: Doc,
) -> JwstResult<Doc> {
    {
        let mut trx = doc.transact_mut();
        for record in update_records {
            let id = record.created_at;
            match Update::decode_v1(&record.blob) {
                Ok(update) => {
                    if let Err(e) = catch_unwind(AssertUnwindSafe(|| trx.apply_update(update))) {
                        warn!("update {} merge failed, skip it: {:?}", id, e);
                    }
                }
                Err(err) => warn!("failed to decode update: {:?}", err),
            }
        }
        trx.commit();
    }

    trace!(
        "migrate_update: {:?}",
        doc.transact()
            .encode_state_as_update_v1(&StateVector::default())?
    );

    Ok(doc)
}

pub fn merge_doc_records(
    update_records: Vec<<Docs as EntityTrait>::Model>,
) -> JwstStorageResult<Vec<u8>> {
    let state_vector = migrate_update(update_records, Doc::default()).and_then(|doc| {
        Ok(doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())?)
    })?;

    Ok(state_vector)
}
