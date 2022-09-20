use super::*;
use crate::sync::*;
use dashmap::mapref::entry::Entry;
use std::sync::Arc;
use yrs::{Doc, Options};

pub async fn create_doc(context: Arc<Context>, workspace: String) -> Doc {
    let doc = Doc::with_options(Options {
        skip_gc: true,
        ..Default::default()
    });

    let db = match context.db.entry(workspace.clone()) {
        Entry::Occupied(value) => value.into_ref(),
        Entry::Vacant(entry) => entry.insert(init("jwst", &workspace).await.unwrap()),
    };

    let updates = db.all(0).await;

    let mut txn = doc.transact();
    for update in updates.unwrap() {
        match Update::decode_v1(&update.blob) {
            Ok(update) => txn.apply_update(update),
            Err(err) => info!("failed to decode update: {:?}", err),
        }
    }
    txn.commit();

    doc
}
