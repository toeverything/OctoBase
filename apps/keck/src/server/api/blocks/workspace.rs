use super::*;
use axum::{extract::Path, http::header};

/// Get a exists workspace by id
/// - Return 200 Ok and workspace's data if workspace is exists.
/// - Return 404 Not Found if workspace not exists.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data", body = Workspace),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> impl IntoResponse {
    info!("get_workspace: {}", workspace);
    if let Some(workspace) = context.workspace.get(&workspace) {
        let workspace = workspace.lock().await;
        Json(&*workspace).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Create a workspace by id
/// - Return 200 Ok and workspace's data if init success or workspace is exists.
/// - Return 500 Internal Server Error if init failed.
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Return workspace data", body = Workspace),
        (status = 500, description = "Failed to init a workspace")
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
        Json(&*workspace).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

/// Delete a exists workspace by id
/// - Return 204 No Content if delete successful.
/// - Return 404 Not Found if workspace not exists.
/// - Return 500 Internal Server Error if delete failed.
#[utoipa::path(
    delete,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}",
    params(
        ("workspace", description = "workspace id"),
    ),
    responses(
        (status = 204, description = "Workspace data deleted"),
        (status = 404, description = "Workspace not exists"),
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

/// Get current client id of server
///
/// When server initializes or gets a workspace, before the workspace is deleted or the server is restarted, any http api modifying the workspace will use same client id.
///
/// This interface return the client id that server will used.
#[utoipa::path(
    get,
    tag = "Workspace",
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

/// Get all client ids of a workspace
///
/// This interface returns all client ids that have modified the workspace
///
/// Every client write something into a workspace will generate a unique id.
///
/// For example:
///   - The user initializes a workspace instance on the web page, synchronizes data from the server, and writes some data to the workspace. All modifications to the workspace use the same client id before the instance is destroyed.
///   - The server initializes or gets a workspace. Before the workspace is deleted or the server is restarted, any http api modifying the workspace will use the same client id
#[utoipa::path(
    get,
    tag = "Workspace",
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

/// Get special client id's modification history of a workspace
///
/// If client id set to 0, return all modification history of a workspace.
#[utoipa::path(
    get,
    tag = "Workspace",
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

#[cfg(all(test, feature = "sqlite"))]
mod test {
    use super::*;

    #[tokio::test]
    async fn workspace() {
        use axum_test_helper::TestClient;

        let context = Arc::new(Context::new().await);

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
            resp.json::<schema::Workspace>().await,
            schema::Workspace::default()
        );

        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.json::<schema::Workspace>().await,
            schema::Workspace::default()
        );
    }
}
