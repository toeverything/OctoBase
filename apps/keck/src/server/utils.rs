use super::*;
use crate::sync::*;
use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use dashmap::mapref::entry::Entry;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use yrs::{Doc, Options};

async fn create_doc(context: Arc<Context>, workspace: String) -> (Doc, SQLite) {
    let doc = Doc::with_options(Options {
        skip_gc: true,
        ..Default::default()
    });

    info!("loading updates...");
    let db = init(context.db_conn.clone(), &workspace).await.unwrap();

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

    (doc, db)
}

pub async fn init_doc(context: Arc<Context>, workspace: String) {
    if let Entry::Vacant(entry) = context.doc.entry(workspace.clone()) {
        let (mut doc, db) = create_doc(context.clone(), workspace.clone()).await;

        if let Entry::Vacant(entry) = context.subscribes.entry(workspace.clone()) {
            let sub = doc.observe_update_v1(move |_, e| {
                let db = db.clone();
                let update = e.update.clone();
                tokio::spawn(async move {
                    info!("writing updates...");

                    db.insert(&update).await.unwrap();
                    info!("writing updates...2");
                });
            });
            entry.insert(sub.into());
        }

        entry.insert(Mutex::new(doc));
    };
}

pub fn parse_doc<T>(any: T) -> impl IntoResponse
where
    T: Serialize,
{
    use serde_json::to_string;
    if let Ok(data) = to_string(&any) {
        ([(header::CONTENT_TYPE, "application/json")], data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
