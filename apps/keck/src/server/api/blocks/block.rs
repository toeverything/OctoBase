use axum::{extract::Query, response::Response};
use jwst_core::{constants, Any};
use serde_json::Value as JsonValue;

use super::{schema::InsertChildren, *};

/// Get a `Block` by id
/// - Return 200 and `Block`'s data if `Block` is exists.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace_id}/{block_id}",
    params(
        ("workspace_id", description = "workspace id"),
        ("block_id", description = "block id"),
    ),
    responses(
        (status = 200, description = "Get block", body = Block),
        (status = 404, description = "Workspace or block content not found"),
    )
)]
pub async fn get_block(Extension(context): Extension<Arc<Context>>, Path(params): Path<(String, String)>) -> Response {
    let (ws_id, block) = params;
    info!("get_block: {}, {}", ws_id, block);
    if let Ok(space) = context
        .get_workspace(ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        if let Some(block) = space.get(block) {
            Json(block).into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Create or modify `Block` if block exists with specific id.
/// Note that flavour can only be set when creating a block.
/// - Return 200 and `Block`'s data if `Block`'s content set successful.
/// - Return 404 Not Found if `Workspace` not exists.
#[utoipa::path(
    post,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace_id}/{block_id}/?flavour={flavour}",
    params(
        ("workspace_id", description = "workspace id"),
        ("block_id", description = "block id"),
        ("flavour", Query, description = "block flavour, default flavour is text. Optional"),
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
    query_param: Option<Query<HashMap<String, String>>>,
    Json(payload): Json<JsonValue>,
) -> Response {
    let (ws_id, block_id) = params;
    info!("set_block: {}, {}", ws_id, block_id);
    if let Ok(mut space) = context
        .get_workspace(&ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        let flavour = if let Some(query_map) = query_param {
            query_map
                .get("flavour")
                .map_or_else(|| String::from("text"), |v| v.clone())
        } else {
            String::from("text")
        };

        if let Ok(mut block) = space
            .create(&block_id, flavour)
            .map_err(|e| error!("failed to create block: {:?}", e))
        {
            // set block content
            if let Some(block_content) = payload.as_object() {
                for (key, value) in block_content.iter() {
                    if key == constants::sys::FLAVOUR {
                        continue;
                    }

                    if let Ok(value) = serde_json::from_value::<Any>(value.clone()) {
                        if let Err(e) = block.set(key, value.clone()) {
                            error!(
                                "failed to set block {} content: {}, {:?}, {:?}",
                                block_id, key, value, e
                            );
                        }
                    }
                }
            }

            // response block content
            return Json(block).into_response();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}

/// Get exists `Blocks` in certain `Workspace` by flavour
/// - Return 200 Ok and `Blocks`'s data if `Blocks` is exists.
/// - Return 404 Not Found if `Workspace` not exists or 500 Internal Server
///   Error when transaction init fails.
#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace_id}/flavour/{flavour}",
    params(
        ("workspace_id", description = "workspace id"),
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
    if let Ok(space) = context
        .get_workspace(&ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        Json(space.get_blocks_by_flavour(&flavour)).into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({ws_id:?}) not found")).into_response()
    }
}

/// Delete block
/// - Return 204 No Content if delete successful.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    delete,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace_id}/{block_id}",
    params(
        ("workspace_id", description = "workspace id"),
        ("block_id", description = "block id"),
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
    if let Ok(mut space) = context
        .get_workspace(&ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        space.remove(&block);
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Get children in `Block`
/// - Return 200 and `Block`'s children ID.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace_id}/{block_id}/children",
    params(
        ("workspace_id", description = "workspace id"),
        ("block_id", description = "block id"),
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
    if let Ok(space) = context
        .get_workspace(ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        if let Some(block) = space.get(&block) {
            let data: Vec<String> = block.children_iter(|children| children.skip(offset).take(limit).collect());

            let status = if data.is_empty() {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            };

            return (
                status,
                Json(PageData {
                    total: block.children_len() as usize,
                    data,
                }),
            )
                .into_response();
        }
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
    path = "/{workspace_id}/{block_id}/children",
    params(
        ("workspace_id", description = "workspace id"),
        ("block_id", description = "block id"),
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
    if let Ok(space) = context
        .get_workspace(&ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        if let Some(mut block) = space.get(block) {
            match payload {
                InsertChildren::Push(block_id) => {
                    if let Some(mut child) = space.get(block_id) {
                        if let Err(e) = block.push_children(&mut child) {
                            error!("failed to insert block: {:?}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                }
                InsertChildren::InsertBefore { id, before } => {
                    if let Some(mut child) = space.get(id) {
                        if let Err(e) = block.insert_children_before(&mut child, &before) {
                            error!("failed to insert children before: {:?}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                }
                InsertChildren::InsertAfter { id, after } => {
                    if let Some(mut child) = space.get(id) {
                        if let Err(e) = block.insert_children_after(&mut child, &after) {
                            // TODO: handle error correctly
                            error!("failed to insert children after: {:?}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                }
                InsertChildren::InsertAt { id, pos } => {
                    if let Some(mut child) = space.get(id) {
                        if let Err(e) = block.insert_children_at(&mut child, pos) {
                            // TODO: handle error correctly
                            error!("failed to insert children at: {:?}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                }
            };

            // response block content
            return Json(block).into_response();
        }
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
    path = "/{workspace_id}/{block_id}/children/{children}",
    params(
        ("workspace_id", description = "workspace id"),
        ("block_id", description = "block id"),
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
    if let Ok(space) = context
        .get_workspace(&ws_id)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        if let Some(mut block) = space.get(&block) {
            if block.children_exists(&child_id) {
                if let Some(mut child) = space.get(&child_id) {
                    if let Err(e) = block.remove_children(&mut child) {
                        error!("failed to remove block: {:?}", e);
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    } else {
                        // response block content
                        return Json(block).into_response();
                    }
                }
            }
        }
    }
    StatusCode::NOT_FOUND.into_response()
}
