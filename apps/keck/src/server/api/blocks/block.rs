use super::*;
use jwst::{BlockHistory, InsertChildren, RemoveChildren};

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
    if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.value().lock().await;
        if let Some(block) = workspace.get(block) {
            Json(block).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
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
    if let Some(workspace) = context.workspace.get(&workspace) {
        // init block instance
        let workspace = workspace.lock().await;
        // set block content
        let block = workspace.with_trx(|mut t| {
            let mut block = t.create(&block, "text");
            // set block content
            if let Some(block_content) = payload.as_object() {
                for (key, value) in block_content.iter() {
                    block.set(&mut t.trx, key, value.clone());
                }
            }
            block
        });

        // response block content
        Json(block.block().to_json()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

// get block history
#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/history",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    responses(
        (status = 200, description = "Get block history", body = inline([BlockHistory])),
        (status = 404, description = "Workspace or block not found"),
        (status = 500, description = "Failed to get block history")
    )
)]
pub async fn get_block_history(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("get_block_history: {}, {}", workspace, block);
    if let Some(workspace) = context.workspace.get(&workspace) {
        // init block instance
        let workspace = workspace.value().lock().await;
        if let Some(block) = workspace.get(block) {
            Json(&block.history()).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
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
    if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.value().lock().await;
        if workspace.get_trx().remove(&block) {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        }
    } else {
        StatusCode::NOT_FOUND
    }
}

// insert children block
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
    if let Some(workspace) = context.workspace.get(&workspace) {
        // init block instance
        let workspace = workspace.value().lock().await;
        if let Some(mut block) = workspace.get(block) {
            workspace.with_trx(|mut t| {
                block.insert_children(&mut t.trx, payload);
            });
            // response block content
            Json(block.block().to_json()).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

// remove children block
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
    if let Some(workspace) = context.workspace.get(&workspace) {
        // init block instance
        let workspace = workspace.value().lock().await;
        if let Some(mut block) = workspace.get(&block) {
            workspace.with_trx(|mut t| {
                block.remove_children(&mut t.trx, payload);
            });
            // response block content
            Json(block.block().to_json()).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
