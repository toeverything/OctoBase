
use super::*;



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
            if let Ok(data) = serde_json::to_string(&block.to_json()) {
                ([(header::CONTENT_TYPE, "application/json")], data).into_response()
            } else {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
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
    info!("create_block: {}, {}", workspace, block);
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
        match json_to_string(&block.block().to_json()) {
            Ok(json) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
