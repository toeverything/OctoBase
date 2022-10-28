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
    if let Err(e) = utils::init_doc(context.clone(), &workspace).await {
        error!("Failed to init doc: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    } else if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.lock().await;
        Json(&*workspace).into_response()
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

    if let Err(e) = utils::init_doc(context.clone(), &workspace).await {
        error!("Failed to init doc: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    } else if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.lock().await;
        Json(workspace.doc().transact().get_map("blocks").to_json()).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
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
    if context.workspace.remove(&workspace).is_none() {
        return StatusCode::NOT_FOUND;
    }
    if context.db.drop(&workspace).await.is_err() {
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
        (status = 200, description = "Get workspace client id", body = u64),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn workspace_client(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.lock().await;
        Json(workspace.client_id()).into_response()
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
        (status = 200, description = "Get workspace history client ids", body = [u64]),
        (status = 500, description = "Failed to get workspace history")
    )
)]
pub async fn history_workspace_clients(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.lock().await;
        if let Some(history) = parse_history_client(workspace.doc()) {
            Json(history).into_response()
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
        (status = 200, description = "Get workspace history", body = [RawHistory]),
        (status = 400, description = "Client id invalid"),
        (status = 500, description = "Failed to get workspace history")
    )
)]
pub async fn history_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> impl IntoResponse {
    let (workspace, client) = params;
    if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.lock().await;
        if let Ok(client) = client.parse::<u64>() {
            if let Some(json) = parse_history(workspace.doc(), client)
                .and_then(|history| serde_json::to_string(&history).ok())
            {
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

#[cfg(all(test, not(feature = "mysql")))]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn workspace() {
        use axum_test_helper::TestClient;
        use serde::Deserialize;

        let context = Arc::new(Context::new().await);

        #[derive(Deserialize, PartialEq, Debug)]
        struct WorkspaceStruct {
            content: HashMap<String, ()>,
            updated: HashMap<String, ()>,
        }

        let app = Router::new()
            .route(
                "/block/:workspace",
                get(blocks::get_workspace)
                    .post(blocks::set_workspace)
                    .delete(blocks::delete_workspace),
            )
            .layer(Extension(context));

        let client = TestClient::new(app);

        let resp = client.post("/block/test").send().await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.json::<WorkspaceStruct>().await,
            WorkspaceStruct {
                content: HashMap::new(),
                updated: HashMap::new()
            }
        );

        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.json::<WorkspaceStruct>().await,
            WorkspaceStruct {
                content: HashMap::new(),
                updated: HashMap::new()
            }
        );
    }
}
