use crate::sync::{write_sync, write_update};

use super::*;
use axum::extract::Path;
use dashmap::mapref::entry::Entry;
use yrs::updates::encoder::{Encoder, EncoderV1};

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data")
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> String {
    info!("get_workspace: {}", workspace);
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        trx.get_map("blocks").to_json().to_string()
    } else {
        "".to_owned()
    }
}

#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data")
    )
)]
pub async fn set_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> String {
    info!("get_workspace: {}", workspace);

    let doc = match context.doc.entry(workspace.clone()) {
        Entry::Occupied(value) => value.into_ref(),
        Entry::Vacant(entry) => {
            let doc = utils::create_doc(context.clone(), workspace.clone()).await;
            entry.insert(Mutex::new(doc))
        }
    };
    let mut doc = doc.value().lock().await;

    let context = context.clone();
    let workspace = workspace.clone();
    let sub = doc.observe_update_v1(move |_, e| {
        let mut encoder = EncoderV1::new();
        write_sync(&mut encoder);
        write_update(&e.update, &mut encoder);
        let update = encoder.to_vec();

        let context = context.clone();
        let workspace = workspace.clone();
        tokio::spawn(async move {
            let db = match context.db.entry(workspace.clone()) {
                Entry::Occupied(value) => value.into_ref(),
                Entry::Vacant(entry) => entry.insert(init("jwst", &workspace).await.unwrap()),
            };

            db.insert(&update).await.unwrap();
        });
    });
    std::mem::forget(sub);

    doc.transact().get_map("blocks").to_json().to_string()
}
