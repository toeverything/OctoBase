use crate::sync::SQLite;

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
    utils::init_doc(context.clone(), workspace.clone()).await;

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

    utils::init_doc(context.clone(), workspace.clone()).await;

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
    let conn = if let Ok(conn) = context.db_conn.acquire().await {
        conn
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let mut db = SQLite {
        conn,
        table: workspace.clone(),
    };
    if let Err(_) = db.drop().await {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    StatusCode::NO_CONTENT
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
        (status = 200, description = "Get workspace history", body = inline([BlockHistory])),
        (status = 500, description = "Failed to get workspace history")
    )
)]
pub async fn history_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    if let Some(history) = context.history.get(&workspace) {
        if let Ok(json) = serde_json::to_string(&history.lock().await.iter().collect::<Vec<_>>()) {
            ([(header::CONTENT_TYPE, "application/json")], json).into_response()
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
