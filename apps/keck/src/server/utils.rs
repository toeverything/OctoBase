use super::*;
use crate::sync::*;
use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use dashmap::mapref::entry::Entry;
use serde::Serialize;
use std::{
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};
use tokio::sync::{mpsc::channel, Mutex};
use utoipa::ToSchema;
use yrs::{
    block::{Item, ItemContent, ID},
    types::TypePtr,
    Doc, Options, StateVector,
};

pub enum Migrate {
    Update(Vec<u8>),
    Full(Vec<u8>),
}

#[derive(Debug, Serialize, ToSchema)]
pub struct History {
    id: String,
    parent: String,
    content: String,
}

pub fn parse_history(doc: &Doc) -> Option<String> {
    let update = doc.encode_state_as_update_v1(&StateVector::default());
    if let Ok(update) = Update::decode_v1(&update) {
        let items = update.as_items();

        let mut histories = vec![];
        let mut map: HashMap<ID, String> = HashMap::new();

        let mut parse_parent = |item: &Item| {
            let parent = match &item.parent {
                TypePtr::Unknown => "unknown".to_owned(),
                TypePtr::Branch(ptr) => {
                    if let Some(name) = ptr.item_id().and_then(|id| map.get(&id)) {
                        name.clone()
                    } else {
                        "null".to_owned()
                    }
                }
                TypePtr::Named(name) => name.to_string(),
                TypePtr::ID(id) => {
                    if let Some(name) = map.get(id) {
                        name.clone()
                    } else {
                        "null".to_owned()
                    }
                }
            };

            if ["unknown", "null"].contains(&parent.as_str()) {
                None
            } else {
                let parent = if let Some(parent_sub) = &item.parent_sub {
                    format!("{}.{}", parent, parent_sub)
                } else {
                    parent
                };

                map.insert(item.id, parent.clone());

                Some(parent)
            }
        };

        for item in items {
            if let ItemContent::Deleted(_) = item.content {
                continue;
            }
            if let Some(parent) = parse_parent(item) {
                let id = format!("{}:{}", item.id.clock, item.id.client);
                histories.push(History {
                    id,
                    parent,
                    content: item.content.to_string(),
                })
            }
        }

        serde_json::to_string(&histories).ok()
    } else {
        None
    }
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
