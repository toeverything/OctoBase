use super::*;
use axum::{extract::Path, http::header};

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data"),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    info!("get_workspace: {}", workspace);
    utils::init_doc(context.clone(), &workspace).await;

    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.value().lock().await;
        let mut trx = doc.transact();
        utils::parse_doc(trx.get_map("blocks").to_json()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
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
) -> impl IntoResponse {
    info!("set_workspace: {}", workspace);

    utils::init_doc(context.clone(), &workspace).await;

    let doc = context.doc.get(&workspace).unwrap();
    let doc = doc.lock().await;

    utils::parse_doc(doc.transact().get_map("blocks").to_json()).into_response()
}

#[utoipa::path(
    delete,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 204, description = "Workspace data deleted"),
        (status = 500, description = "Failed to delete workspace")
    )
)]
pub async fn delete_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    info!("delete_workspace: {}", workspace);
    if context.doc.remove(&workspace).is_none() {
        return StatusCode::NOT_FOUND;
    }
    context.subscribes.remove(&workspace);
    if let Err(_) = context.db.drop(&workspace).await {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    StatusCode::NO_CONTENT
}

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/client",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace client id", body = inline(u64)),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn workspace_client(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.lock().await;
        (
            [(header::CONTENT_TYPE, "application/json")],
            doc.client_id.to_string(),
        )
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/history",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace history client ids", body = inline([u64])),
        (status = 500, description = "Failed to get workspace history")
    )
)]
pub async fn history_workspace_clients(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.lock().await;
        if let Some(json) = utils::parse_history_client(&doc) {
            ([(header::CONTENT_TYPE, "application/json")], json).into_response()
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[utoipa::path(
    get,
    tag = "Blocks",
    context_path = "/api/block",
    path = "/{workspace}/history/{client}",
    params(
        ("workspace", description = "workspace id"),
        ("client", description = "client id, is give 0 then return all clients histories"),
    ),
    responses(
        (status = 200, description = "Get workspace history", body = inline([utils::History])),
        (status = 400, description = "Client id invalid"),
        (status = 500, description = "Failed to get workspace history")
    )
)]
pub async fn history_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, client) = params;
    if let Some(doc) = context.doc.get(&workspace) {
        let doc = doc.lock().await;
        if let Ok(client) = client.parse::<u64>() {
            if let Some(json) = utils::parse_history(&doc, client) {
                ([(header::CONTENT_TYPE, "application/json")], json).into_response()
            } else {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        } else {
            StatusCode::BAD_REQUEST.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
