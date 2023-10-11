use jwst_codec::{Doc, StateVector};

use super::{entities::prelude::*, types::JwstStorageResult, *};

// apply all updates to the given doc
pub fn migrate_update(update_records: Vec<<Docs as EntityTrait>::Model>, mut doc: Doc) -> JwstResult<Doc> {
    // stop update dispatch before apply updates
    doc.publisher.stop();
    for record in update_records {
        let id = record.created_at;
        if let Err(e) = doc.apply_update_from_binary(record.blob) {
            warn!("update {} merge failed, skip it: {:?}", id, e);
        }
    }
    // temporarily disable due to the multiple client issue
    // doc.gc()?;

    doc.publisher.start();

    Ok(doc)
}

pub fn merge_doc_records(update_records: Vec<<Docs as EntityTrait>::Model>) -> JwstStorageResult<Vec<u8>> {
    let doc = migrate_update(update_records, Doc::default())?;
    let state_vector = doc.encode_state_as_update_v1(&StateVector::default())?;

    Ok(state_vector)
}
