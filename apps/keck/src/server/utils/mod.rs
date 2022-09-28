mod history;

pub use history::{parse_history, History};

use super::*;
use crate::sync::*;
use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use dashmap::mapref::entry::Entry;
use serde::Serialize;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};
use tokio::sync::{mpsc::channel, Mutex};
use yrs::{Doc, Options, StateVector};

pub enum Migrate {
    Update(Vec<u8>),
    Full(Vec<u8>),
}

fn migrate_update(updates: Vec<UpdateBinary>, doc: Doc) -> Doc {
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

    doc
}

async fn load_doc(db: &mut SQLite) -> Result<Doc, sqlx::Error> {
    let mut doc = Doc::with_options(Options {
        skip_gc: true,
        ..Default::default()
    });

    let all_data = db.all().await?;

    if all_data.is_empty() {
        let update = doc.encode_state_as_update_v1(&StateVector::default());
        db.insert(&update).await?;
    } else {
        doc = migrate_update(all_data, doc);
    }

    Ok(doc)
}

async fn create_doc(context: Arc<Context>, workspace: String) -> Doc {
    let mut db = init(context.db_conn.clone(), &workspace).await.unwrap();
    let doc = load_doc(&mut db).await.unwrap();

    doc
}

pub async fn init_doc(context: Arc<Context>, workspace: String) {
    if let Entry::Vacant(entry) = context.doc.entry(workspace.clone()) {
        let doc = create_doc(context.clone(), workspace.clone()).await;
        let (tx, mut rx) = channel::<Migrate>(100);

        {
            // storage thread
            let context = context.clone();
            let workspace = workspace.clone();
            tokio::spawn(async move {
                while let Some(update) = rx.recv().await {
                    let conn = context.db_conn.acquire().await;
                    if let Ok(conn) = conn {
                        let mut db = SQLite {
                            conn,
                            table: workspace.clone(),
                        };
                        let res = match update {
                            Migrate::Update(update) => {
                                db.update(update, |all_data| {
                                    let doc = migrate_update(all_data, Doc::default());

                                    doc.encode_state_as_update_v1(&StateVector::default())
                                })
                                .await
                            }
                            Migrate::Full(full) => db.full_migrate(full).await,
                        };
                        if let Err(e) = res {
                            error!("failed to update document: {:?}", e);
                        }
                    } else {
                        error!("get DB connection failed")
                    }
                }
                info!("storage final: {}", workspace);
            });
        }

        context.storage.insert(workspace.clone(), tx);

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
