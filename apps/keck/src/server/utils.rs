use super::*;
use crate::sync::*;
use dashmap::mapref::entry::Entry;
use std::sync::Arc;
use tokio::sync::Mutex;
use yrs::{Doc, Options};

async fn create_doc(context: Arc<Context>, workspace: String) -> Doc {
    let doc = Doc::with_options(Options {
        skip_gc: true,
        ..Default::default()
    });

    info!("loading updates...");
    let db = match context.db.entry(workspace.clone()) {
        Entry::Occupied(value) => {
            info!("loading updates...1.1");
            value.into_ref()
        }
        Entry::Vacant(entry) => {
            info!("loading updates...1.2");
            entry.insert(init(context.db_conn.clone(), &workspace).await.unwrap())
        }
    };

    info!("loading updates... 2");
    let updates = db.all(0).await;

    info!("loading updates...3");

    let mut txn = doc.transact();
    for update in updates.unwrap() {
        match Update::decode_v1(&update.blob) {
            Ok(update) => txn.apply_update(update),
            Err(err) => info!("failed to decode update: {:?}", err),
        }
    }
    txn.commit();

    info!("loading updates...4");

    doc
}

pub async fn init_doc(context: Arc<Context>, workspace: String) {
    if let Entry::Vacant(entry) = context.doc.entry(workspace.clone()) {
        let mut doc = create_doc(context.clone(), workspace.clone()).await;

        if let Entry::Vacant(entry) = context.subscribes.entry(workspace.clone()) {
            let context = context.clone();
            let workspace = workspace.clone();
            let sub = doc.observe_update_v1(move |_, e| {
                let context = context.clone();
                let workspace = workspace.clone();
                let update = e.update.clone();
                tokio::spawn(async move {
                    info!("writing updates...");
                    let db = match context.db.entry(workspace.clone()) {
                        Entry::Occupied(value) => {
                            info!("writing updates...1.1");
                            value.into_ref()
                        }
                        Entry::Vacant(entry) => {
                            info!("writing updates...1.2");
                            entry.insert(init(context.db_conn.clone(), &workspace).await.unwrap())
                        }
                    };
                    info!("writing updates...2");

                    db.insert(&update).await.unwrap();
                    info!("writing updates...3");
                });
            });
            entry.insert(sub.into());
        }

        entry.insert(Mutex::new(doc));
    };
}
