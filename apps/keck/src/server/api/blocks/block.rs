use super::*;
use jwst::{InsertChildren, RemoveChildren};

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    responses(
        (status = 200, description = "Get block"),
        (status = 404, description = "Workspace or block content not found"),
        (status = 500, description = "Block data error")
    )
)]
pub async fn get_block(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("get_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        if let Some(block) = trx
            .get_map("blocks")
            .get("content")
            .and_then(|b| b.to_ymap())
            .and_then(|b| b.get(&block))
        {
            utils::parse_doc(block.to_json()).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

fn set_value(block: &mut Map, trx: &mut Transaction, key: &str, value: &JsonValue) {
    match value {
        JsonValue::Bool(v) => {
            block.insert(trx, key.clone(), *v);
        }
        JsonValue::Null => {
            block.remove(trx, key);
        }
        JsonValue::Number(v) => {
            if let Some(v) = v.as_f64() {
                block.insert(trx, key.clone(), v);
            } else if let Some(v) = v.as_i64() {
                block.insert(trx, key.clone(), v);
            } else if let Some(v) = v.as_u64() {
                block.insert(trx, key.clone(), i64::try_from(v).unwrap_or(0));
            }
        }
        JsonValue::String(v) => {
            block.insert(trx, key.clone(), v.clone());
        }
        _ => {}
    }
}

// create or set block with content
#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    request_body(
        content = String,
        description = "json",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Block created and content was set"),
        (status = 404, description = "Workspace not found"),
        (status = 500, description = "Failed to create block")
    )
)]
pub async fn set_block(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<JsonValue>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("set_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        // init block instance
        let mut trx = doc.value().lock().await.transact();
        let mut block = Block::new(&mut trx, &block, "text");

        // set block content
        if let Some(block_content) = payload.as_object() {
            for (key, value) in block_content.iter() {
                set_value(block.content(), &mut trx, key, value);
            }
        }

        // response block content
        utils::parse_doc(block.block().to_json()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

// delete block
#[utoipa::path(
    delete,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    responses(
        (status = 204, description = "Block successfully deleted"),
        (status = 404, description = "Workspace or block not found"),
    )
)]
pub async fn delete_block(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("delete_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        if let Some(_) = trx
            .get_map("blocks")
            .get("content")
            .and_then(|b| b.to_ymap())
            .and_then(|b| b.remove(&mut trx, &block))
        {
            trx.commit();
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        }
    } else {
        StatusCode::NOT_FOUND
    }
}

// insert block
#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/insert",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    request_body(
        content = inline(InsertChildren),
        description = "json",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Block inserted"),
        (status = 404, description = "Workspace or block not found"),
        (status = 500, description = "Failed to insert block")
    )
)]
pub async fn insert_block(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<InsertChildren>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("insert_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        // init block instance
        let mut trx = doc.value().lock().await.transact();
        if let Some(mut block) = Block::from(&mut trx, &block) {
            block.insert_children(&mut trx, payload);
            // response block content
            utils::parse_doc(block.block().to_json()).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

// remove block
#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/remove",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    request_body(
        content = inline(RemoveChildren),
        description = "json",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Block children removed"),
        (status = 404, description = "Workspace or block not found"),
        (status = 500, description = "Failed to remove block children")
    )
)]
pub async fn remove_block(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<RemoveChildren>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("insert_block: {}, {}", workspace, block);
    if let Some(doc) = context.doc.get(&workspace) {
        // init block instance
        let mut trx = doc.value().lock().await.transact();
        if let Some(mut block) = Block::from(&mut trx, &block) {
            block.remove_children(&mut trx, payload);
            // response block content
            utils::parse_doc(block.block().to_json()).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
