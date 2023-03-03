use super::*;
use axum::{extract::Query, response::Response};
use jwst::DocStorage;
use lib0::any::Any;
use serde_json::{Map, Value};

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
    let (ws_id, block_id) = params;
    info!("get_block: {}, {}", ws_id, block_id);

    if let Some(block) = context
        .storage
        .get_workspace(ws_id)
        .await
        .ok()
        .and_then(|workspace| workspace.with_trx(|t| workspace.get(&t.trx, block_id)))
    {
        return Json(block).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
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
    Path(params): Path<(String, String)>,
    Json(payload): Json<Map<String, Value>>,
) -> Response {
    let (ws_id, block_id) = params;
    info!("set_block: {}, {}", ws_id, block_id);

    if let Some((update, block)) =
        context
            .storage
            .get_workspace(&ws_id)
            .await
            .ok()
            .and_then(|workspace| {
                workspace.with_trx(|mut t| {
                    let block = t.create(&block_id, "text");

                    let count = payload
                        .into_iter()
                        .filter_map(|(key, value)| {
                            serde_json::from_value::<Any>(value)
                                .map(|value| block.set(&mut t.trx, &key, value))
                                .ok()
                        })
                        .count();

                    Some(((count > 0).then(|| t.trx.encode_update_v1()), block))
                })
            })
    {
        if let Some(update) = update {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
        }

        // response block content
        return Json(block).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
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
    let (ws_id, block_id) = params;
    info!("get_block_history: {}, {}", ws_id, block_id);

    if let Some(history) = context
        .storage
        .get_workspace(&ws_id)
        .await
        .ok()
        .and_then(|workspace| {
            workspace.with_trx(|t| {
                workspace
                    .get(&t.trx, block_id)
                    .map(|block| block.history(&t.trx))
            })
        })
    {
        return Json(history).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
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
    let (ws_id, block_id) = params;
    info!("delete_block: {}, {}", ws_id, block_id);

    if let Some(update) = context
        .storage
        .get_workspace(&ws_id)
        .await
        .ok()
        .and_then(|workspace| {
            workspace.with_trx(|mut t| t.remove(&block_id).then(|| t.trx.encode_update_v1()))
        })
    {
        if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
            error!("db write error: {}", e.to_string());
        }
        return StatusCode::NO_CONTENT;
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
    let (ws_id, block_id) = params;
    let Pagination { offset, limit } = pagination;
    info!("get_block_children: {}, {}", ws_id, block_id);

    if let Some((total, data)) = context
        .storage
        .get_workspace(ws_id)
        .await
        .ok()
        .and_then(|workspace| workspace.with_trx(|t| workspace.get(&t.trx, &block_id)))
        .map(|block| {
            (
                block.children_len() as usize,
                block.children_iter(|children| {
                    children.skip(offset).take(limit).collect::<Vec<String>>()
                }),
            )
        })
    {
        return (
            if data.is_empty() {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            },
            Json(PageData { total, data }),
        )
            .into_response();
    }

    StatusCode::NOT_FOUND.into_response()
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
    Json(action): Json<InsertChildren>,
) -> Response {
    let (ws_id, block_id) = params;
    info!("insert_block: {}, {}", ws_id, block_id);

    if let Some((update, block)) =
        context
            .storage
            .get_workspace(&ws_id)
            .await
            .ok()
            .and_then(|workspace| {
                workspace.with_trx(|mut t| {
                    workspace.get(&t.trx, block_id).map(|block| {
                        (
                            match action {
                                InsertChildren::Push(id) => workspace
                                    .get(&t.trx, id)
                                    .map(|child| block.push_children(&mut t.trx, &child)),
                                InsertChildren::InsertBefore { id, before } => {
                                    workspace.get(&t.trx, id).map(|child| {
                                        block.insert_children_before(&mut t.trx, &child, &before)
                                    })
                                }
                                InsertChildren::InsertAfter { id, after } => {
                                    workspace.get(&t.trx, id).map(|child| {
                                        block.insert_children_after(&mut t.trx, &child, &after)
                                    })
                                }
                                InsertChildren::InsertAt { id, pos } => workspace
                                    .get(&t.trx, id)
                                    .map(|child| block.insert_children_at(&mut t.trx, &child, pos)),
                            }
                            .map(|_| t.trx.encode_update_v1()),
                            block,
                        )
                    })
                })
            })
    {
        if let Some(update) = update {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
        }

        // response block content
        return Json(block).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
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
    let (ws_id, block_id, child_id) = params;
    info!("remove_block: {}, {}, {}", ws_id, block_id, child_id);

    if let Some((update, block)) =
        context
            .storage
            .get_workspace(&ws_id)
            .await
            .ok()
            .and_then(|workspace| {
                workspace.with_trx(|mut t| {
                    workspace.get(&t.trx, &block_id).map(|block| {
                        (
                            block
                                .children_exists(&t.trx, &child_id)
                                .then(|| {
                                    workspace.get(&t.trx, &child_id).map(|child| {
                                        block.remove_children(&mut t.trx, &child);
                                        t.trx.encode_update_v1()
                                    })
                                })
                                .flatten(),
                            block,
                        )
                    })
                })
            })
    {
        if let Some(update) = update {
            if let Err(e) = context.storage.docs().write_update(ws_id, &update).await {
                error!("db write error: {}", e.to_string());
            }
        }

        // response block content
        return Json(block).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
