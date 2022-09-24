use super::*;
use crate::sync::*;
use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use dashmap::mapref::entry::Entry;
use serde::Serialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tokio::sync::{mpsc::channel, Mutex};
use yrs::{
    types::{DeepObservable, PathSegment},
    Doc, Options, StateVector,
};

pub enum Migrate {
    Update(Vec<u8>),
    Full(Vec<u8>),
}

const MAX_TRIM_UPDATE_LIMIT: i64 = 500;

async fn migrate_update(db: &SQLite, doc: Doc) -> Result<Doc, sqlx::Error> {
    let updates = db.all(0).await?;

    let mut trx = doc.transact();
    for update in updates {
        let id = update.id;
        match Update::decode_v1(&update.blob) {
            Ok(update) => {
                if let Err(e) = catch_unwind(AssertUnwindSafe(|| trx.apply_update(update))) {
                    warn!("update {} merge failed, skip it: {:?}", id, e);
                }
            }
            Err(err) => info!("failed to decode update: {:?}", err),
        }
    }
    trx.commit();

    Ok(doc)
}

async fn load_doc(db: &SQLite) -> Result<Doc, sqlx::Error> {
    let mut doc = Doc::with_options(Options {
        skip_gc: true,
        ..Default::default()
    });

    if db.count().await? == 0 {
        let update = doc.encode_state_as_update_v1(&StateVector::default());
        db.insert(&update).await?;
    } else {
        doc = migrate_update(db, doc).await?;
    }

    Ok(doc)
}

async fn update_document(db: &SQLite, update: Migrate) -> Result<(), sqlx::Error> {
    match update {
        Migrate::Update(update) => {
            db.insert(&update).await.unwrap();
            if db.count().await.unwrap() > MAX_TRIM_UPDATE_LIMIT {
                let doc = migrate_update(&db, Doc::default()).await?;

                let update = doc.encode_state_as_update_v1(&StateVector::default());
                db.insert(&update).await?;

                let clock = db.max_id().await?;
                db.delete_before(clock).await?;
            }
        }
        Migrate::Full(full) => {
            if db.count().await? > 1 {
                db.insert(&full).await?;

                let clock = db.max_id().await?;
                db.delete_before(clock).await?;

                info!("full refresh: {}", db.table);
            }
        }
    }
    Ok(())
}

async fn create_doc(context: Arc<Context>, workspace: String) -> (Doc, SQLite) {
    let db = init(context.db_conn.clone(), &workspace).await.unwrap();
    let doc = load_doc(&db).await.unwrap();

    (doc, db)
}

pub async fn init_doc(context: Arc<Context>, workspace: String) {
    if let Entry::Vacant(entry) = context.doc.entry(workspace.clone()) {
        let (doc, db) = create_doc(context.clone(), workspace.clone()).await;
        let (tx, mut rx) = channel::<Migrate>(100);
        let (history_tx, mut history_rx) = channel::<BlockHistory>(100);

        {
            // storage thread
            let workspace = workspace.clone();
            tokio::spawn(async move {
                while let Some(update) = rx.recv().await {
                    if let Err(e) = update_document(&db, update).await {
                        error!("failed to update document: {:?}", e);
                    }
                }
                info!("storage final: {}", workspace);
            });
        }

        let history = Arc::new(Mutex::new(vec![]));
        let history_clone = history.clone();
        tokio::spawn(async move {
            while let Some(h) = history_rx.recv().await {
                debug!("change: {:?}", h.path);
                history_clone.lock().await.push(h);
            }
        });

        context.history.insert(workspace.clone(), history.clone());
        context.storage.insert(workspace.clone(), tx);

        let history = Arc::new(Mutex::new(history_tx));
        if let Entry::Vacant(entry) = context.subscribes.entry(workspace.clone()) {
            let blocks = doc.transact().get_map("blocks");

            if let Some(mut blocks) = blocks.get("content").and_then(|c| c.to_ymap()) {
                let sub = blocks.observe_deep(move |_, e| {
                    let paths = e
                        .iter()
                        .map(|e| {
                            e.path()
                                .into_iter()
                                .map(|p| match p {
                                    PathSegment::Key(k) => k.as_ref().to_string(),
                                    PathSegment::Index(i) => i.to_string(),
                                })
                                .collect::<Vec<String>>()
                                .join("/")
                        })
                        .collect::<Vec<_>>();
                    let history = history.clone();
                    tokio::spawn(async move {
                        for path in &paths {
                            if !path.is_empty() {
                                if let Err(e) = history
                                    .lock()
                                    .await
                                    .send(BlockHistory::new(path.clone()))
                                    .await
                                {
                                    warn!("Failed to log history: {}, {:?}", path, e);
                                }
                            }
                        }
                        debug!("record {} history", paths.len());
                    });
                });
                entry.insert(sub.into());
            }
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

mod tests {
    use super::*;
    #[tokio::test]
    async fn doc_load_test() -> anyhow::Result<()> {
        let doc = Doc::default();

        {
            let mut trx = doc.transact();
            let mut block = jwst::Block::new(&mut trx, "test", "text");
            block.content().insert(&mut trx, "test", "test");
            trx.commit();
        }

        let new_doc = {
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            let doc = Doc::default();
            let mut trx = doc.transact();
            match Update::decode_v1(&update) {
                Ok(update) => trx.apply_update(update),
                Err(err) => info!("failed to decode update: {:?}", err),
            }
            trx.commit();
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("blocks").to_json(),
            new_doc.transact().get_map("blocks").to_json()
        );

        Ok(())
    }
}
