use super::*;
use axum::{extract::Query, response::Response};
use jwst::DocStorage;
use lib0::any::Any;
use serde_json::Value as JsonValue;

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
) -> Response {
    let (ws_id, block) = params;
    info!("get_block: {}, {}", ws_id, block);
    if let Ok(workspace) = context.storage.get_workspace(ws_id).await {
        if let Some(block) = workspace.with_trx(|t| workspace.get(&t.trx, block)) {
            Json(block).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Create or set `Block` with id
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
    Path(params): Path<(String, String)>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let (ws_id, block_id) = params;
    info!("set_block: {}, {}", ws_id, block_id);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        let mut update = None;

        // set block content
        let block = workspace.with_trx(|mut t| {
            let block = t.create(&block_id, "text");

            // set block content
            if let Some(block_content) = payload.as_object() {
                let mut changed = false;
                for (key, value) in block_content.iter() {
                    changed = true;
                    if let Ok(value) = serde_json::from_value::<Any>(value.clone()) {
                        block.set(&mut t.trx, key, value);
                    }
                }

                if changed {
                    update = Some(t.trx.encode_update_v1());
                }
            }

            block
        });

        if let Some(update) = update {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
        }

        // response block content
        Json(block).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Create or set `Block` with id and flavour
/// - Return 200 and `Block`'s data if `Block`'s created successful.
/// - Return 404 Not Found if `Workspace` not exists.
#[utoipa::path(
    patch,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/",
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
        (status = 200, description = "Block created", body = Block),
        (status = 404, description = "Workspace not found"),
    )
)]
pub async fn set_block_with_flavour(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
    Json(payload): Json<JsonValue>
) -> Response {
    let (ws_id, block_id) = params;

    let flavour = payload.as_object().map_or_else(|| "text", |block_props_map| {
        block_props_map.get("flavour").map_or_else(|| "text", |req_flavour| {
            req_flavour.as_str().unwrap_or_else(|| {
                warn!("Haven't find flavour in request body, use default flavour: text");
                "text"
            })
        })
    });

    info!("set_block_by_flavour: ws_id, {}, block_id, {}, flavour, {}", ws_id, block_id, flavour);

    let mut update = None;
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        let block = workspace.with_trx(|mut t| {
            let created_block = t.create(block_id, flavour);
            update = Some(t.trx.encode_update_v1());
            created_block
        });
        if let Some(update) = update {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
        }

        Json(block).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        ).into_response()
    }
}

/// Get exists `Blocks` in certain `Workspace` by flavour
/// - Return 200 Ok and `Blocks`'s data if `Blocks` is exists.
/// - Return 404 Not Found if `Workspace` not exists or 500 Internal Server Error when transaction init fails.
#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/flavour/{flavour}",
    params(
        ("workspace", description = "workspace id"),
        ("flavour", description = "block flavour"),
    ),
    responses(
        (status = 200, description = "Get all certain flavour blocks belongs to the given workspace"),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_block_by_flavour(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> Response {
    let (ws_id, flavour) = params;
    info!("get_block_by_flavour: ws_id, {}, flavour, {}", ws_id, flavour);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        match workspace.try_with_trx(|trx| {
            workspace.get_blocks_by_flavour(&trx.trx, &flavour)
        }) {
            Some(blocks) => {
                Json(blocks).into_response()
            },
            None => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Workspace({ws_id:?}) get transaction error"),
                ).into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
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
) -> Response {
    let (ws_id, block) = params;
    info!("get_block_history: {}, {}", ws_id, block);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        workspace.with_trx(|t| {
            if let Some(block) = workspace.get(&t.trx, block) {
                Json(&block.history(&t.trx)).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        })
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
) -> StatusCode {
    let (ws_id, block) = params;
    info!("delete_block: {}, {}", ws_id, block);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        if let Some(update) = workspace.with_trx(|mut t| {
            if t.remove(&block) {
                Some(t.trx.encode_update_v1())
            } else {
                None
            }
        }) {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
            return StatusCode::NO_CONTENT;
        }
    }
    StatusCode::NOT_FOUND
}

/// Get children in `Block`
/// - Return 200 and `Block`'s children ID.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/{block}/children",
    params(
        ("workspace", description = "workspace id"),
        ("block", description = "block id"),
        Pagination
    ),
    responses(
        (status = 200, description = "Get block children", body = PageData<[String]>),
        (status = 404, description = "Workspace or block not found"),
    )
)]
pub async fn get_block_children(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
    Query(pagination): Query<Pagination>,
) -> Response {
    let (ws_id, block) = params;
    let Pagination { offset, limit } = pagination;
    info!("get_block_children: {}, {}", ws_id, block);
    if let Ok(workspace) = context.storage.get_workspace(ws_id).await {
        if let Some(block) = workspace.with_trx(|t| workspace.get(&t.trx, &block)) {
            let data: Vec<String> =
                block.children_iter(|children| children.skip(offset).take(limit).collect());

            let status = if data.is_empty() {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            };

            (
                status,
                Json(PageData {
                    total: block.children_len() as usize,
                    data,
                }),
            )
                .into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
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
    Path(params): Path<(String, String)>,
    Json(payload): Json<InsertChildren>,
) -> Response {
    let (ws_id, block) = params;
    info!("insert_block: {}, {}", ws_id, block);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        let mut update = None;

        if let Some(block) = workspace.with_trx(|t| workspace.get(&t.trx, block)) {
            let block = workspace.with_trx(|mut t| {
                let mut changed = false;
                match payload {
                    InsertChildren::Push(block_id) => {
                        if let Some(child) = workspace.get(&t.trx, block_id) {
                            changed = true;
                            block.push_children(&mut t.trx, &child)
                        }
                    }
                    InsertChildren::InsertBefore { id, before } => {
                        if let Some(child) = workspace.get(&t.trx, id) {
                            changed = true;
                            block.insert_children_before(&mut t.trx, &child, &before)
                        }
                    }
                    InsertChildren::InsertAfter { id, after } => {
                        if let Some(child) = workspace.get(&t.trx, id) {
                            changed = true;
                            block.insert_children_after(&mut t.trx, &child, &after)
                        }
                    }
                    InsertChildren::InsertAt { id, pos } => {
                        if let Some(child) = workspace.get(&t.trx, id) {
                            changed = true;
                            block.insert_children_at(&mut t.trx, &child, pos)
                        }
                    }
                }

                if changed {
                    update = Some(t.trx.encode_update_v1());
                }

                block
            });

            if let Some(update) = update {
                if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                    error!("db write error: {}", e.to_string());
                }
            }

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
) -> Response {
    let (ws_id, block, child_id) = params;
    info!("insert_block: {}, {}", ws_id, block);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        if let Some(update) = workspace.with_trx(|mut t| {
            if let Some(block) = workspace.get(&t.trx, &block) {
                if block.children_exists(&t.trx, &child_id) {
                    if let Some(child) = workspace.get(&t.trx, &child_id) {
                        block.remove_children(&mut t.trx, &child);
                        return Some(t.trx.encode_update_v1());
                    }
                }
            }
            None
        }) {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
            // response block content
            Json(block).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
