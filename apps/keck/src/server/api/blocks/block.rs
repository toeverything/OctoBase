use super::*;
use lib0::any::Any;

/// Get a `Block` by id
/// - Return 200 and `Block`'s data if `Block` is exists.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
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
        (status = 200, description = "Get block", body = Block),
        (status = 404, description = "Workspace or block content not found"),
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

/// Create or set `Block` with content
/// - Return 200 and `Block`'s data if `Block`'s content set successful.
/// - Return 404 Not Found if `Workspace` not exists.
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
        (status = 200, description = "Block created and content was set", body = Block),
        (status = 404, description = "Workspace not found"),
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
            let block = t.create(&block, "text");
            // set block content
            if let Some(block_content) = payload.as_object() {
                for (key, value) in block_content.iter() {
                    if let Ok(value) = serde_json::from_value::<Any>(value.clone()) {
                        block.set(&mut t.trx, key, value);
                    }
                }
            }
            block
        });

        // response block content
        Json(block).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Get `Block` history
/// - Return 200 and `Block`'s history if `Block` exists.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
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
        (status = 200, description = "Get block history", body = [BlockHistory]),
        (status = 404, description = "Workspace or block not found"),
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

/// Delete block
/// - Return 204 No Content if delete successful.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
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

/// Insert a another `Block` into a `Block`'s children
/// - Return 200 and `Block`'s data if insert successful.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/children",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    request_body(
        content = InsertChildren,
        description = "json",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Block inserted", body = Block),
        (status = 404, description = "Workspace or block not found"),
        (status = 500, description = "Failed to insert block")
    )
)]
pub async fn insert_block_children(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<InsertChildren>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, block) = params;
    info!("insert_block: {}, {}", workspace, block);
    if let Some(workspace) = context.workspace.get(&workspace) {
        // init block instance
        let workspace = workspace.value().lock().await;
        if let Some(block) = workspace.get(block) {
            workspace.with_trx(|mut t| match payload {
                InsertChildren::Push(block_id) => {
                    if let Some(child) = workspace.get(&block_id) {
                        block.push_children(&mut t.trx, &child)
                    }
                }
                InsertChildren::InsertBefore { id, before } => {
                    if let Some(child) = workspace.get(&id) {
                        block.insert_children_before(&mut t.trx, &child, &before)
                    }
                }
                InsertChildren::InsertAfter { id, after } => {
                    if let Some(child) = workspace.get(&id) {
                        block.insert_children_after(&mut t.trx, &child, &after)
                    }
                }
                InsertChildren::InsertAt { id, pos } => {
                    if let Some(child) = workspace.get(&id) {
                        block.insert_children_at(&mut t.trx, &child, pos)
                    }
                }
            });
            // response block content
            Json(block).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Remove children in `Block`
/// - Return 200 and `Block`'s data if remove successful.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    delete,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/children/{children}",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
    ),
    responses(
        (status = 200, description = "Block children removed", body = Block),
        (status = 404, description = "Workspace or block not found"),
    )
)]
pub async fn remove_block_children(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String, String)>,
) -> impl IntoResponse {
    let (workspace, block, child_id) = params;
    info!("insert_block: {}, {}", workspace, block);
    if let Some(workspace) = context.workspace.get(&workspace) {
        // init block instance
        let workspace = workspace.value().lock().await;
        if let Some(block) = workspace.get(&block) {
            workspace.with_trx(|mut t| {
                if let Some(child) = workspace.get(&child_id) {
                    block.remove_children(&mut t.trx, &child);
                }
            });
            // response block content
            Json(block).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
