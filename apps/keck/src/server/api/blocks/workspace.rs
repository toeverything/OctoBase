use super::*;
use axum::{
    extract::{Path, Query},
    http::header,
    response::Response,
};
use jwst::{parse_history, parse_history_client, DocStorage};
use reqwest::Client;
use utoipa::IntoParams;

/// Get a exists `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if `Workspace` is exists.
/// - Return 404 Not Found if `Workspace` not exists.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data", body = Workspace),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
) -> Response {
    info!("get_workspace: {}", ws_id);
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        Json(workspace).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

/// Create a `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if init success or `Workspace` is exists.
/// - Return 500 Internal Server Error if init failed.
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Return workspace data", body = Workspace),
        (status = 500, description = "Failed to init a workspace")
    )
)]
pub async fn set_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
) -> Response {
    info!("set_workspace: {}", workspace);

    match context.storage.create_workspace(workspace).await {
        Ok(workspace) => Json(workspace).into_response(),
        Err(e) => {
            error!("Failed to init doc: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Delete a exists `Workspace` by id
/// - Return 204 No Content if delete successful.
/// - Return 404 Not Found if `Workspace` not exists.
/// - Return 500 Internal Server Error if delete failed.
#[utoipa::path(
    delete,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
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
) -> Response {
    info!("delete_workspace: {}", workspace);
    if context.storage.docs().delete(workspace).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    StatusCode::NO_CONTENT.into_response()
}

/// Get current client id of server
///
/// When the server initializes or get the `Workspace`, a `Client` will be created. This `Client` will not be destroyed until the server restarts.
/// Therefore, the `Client ID` in the history generated by modifying `Block` through HTTP API will remain unchanged until the server restarts.
///
/// This interface return the client id that server will used.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}/client",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace client id", body = u64),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn workspace_client(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
) -> Response {
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        Json(workspace.client_id()).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

/// Block search query
// See doc for using utoipa search queries example here: https://github.com/juhaku/utoipa/blob/6c7f6a2d/examples/todo-axum/src/main.rs#L124-L130
#[derive(Deserialize, IntoParams)]
pub struct BlockSearchQuery {
    /// Search by title and text.
    query: String,
}

/// Search workspace blocks of server
///
/// This will return back a list of relevant blocks.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/search",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
        BlockSearchQuery,
    ),
    responses(
        (status = 200, description = "Search results", body = SearchResults),
    )
)]
pub async fn workspace_search(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
    query: Query<BlockSearchQuery>,
) -> Response {
    let query_text = &query.query;
    info!("workspace_search: {ws_id:?} query = {query_text:?}");
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        match workspace.search(query_text) {
            Ok(list) => {
                debug!("workspace_search: {ws_id:?} query = {query_text:?}; {list:#?}");
                Json(list).into_response()
            }
            Err(err) => {
                error!("Internal server error calling workspace_search: {err:?}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
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

#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/search",
    path = "/{workspace_id}/index",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "result", body = Vec<String>),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn get_search_index(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
) -> Response {
    info!("get_search_index: {ws_id:?}");

    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        Json(workspace.metadata().search_index).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/search",
    path = "/{workspace_id}/index",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "success"),
        (status = 400, description = "Bad Request"),
        (status = 404, description = "Workspace not found")
    )
)]
pub async fn set_search_index(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
    Json(fields): Json<Vec<String>>,
) -> Response {
    info!("set_search_index: {ws_id:?} fields = {fields:?}");

    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        if let Ok(true) = workspace.set_search_index(fields) {
            StatusCode::OK.into_response()
        } else {
            StatusCode::BAD_REQUEST.into_response()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

/// Get `Block` in `Workspace`
/// - Return 200 and `Block`'s ID.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}/blocks",
    params(
        ("workspace_id", description = "workspace id"),
        Pagination
    ),
    responses(
        (status = 200, description = "Get Blocks", body = PageData<[Block]>),
        (status = 404, description = "Workspace or block not found"),
    )
)]
pub async fn get_workspace_block(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
    Query(pagination): Query<Pagination>,
) -> Response {
    let Pagination { offset, limit } = pagination;
    info!("get_workspace_block: {ws_id:?}");
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        let (total, data) = workspace.with_trx(|mut t| {
            let space = t.get_blocks();

            let total = space.block_count() as usize;
            let data = space.blocks(&t.trx, |blocks| {
                blocks.skip(offset).take(limit).collect::<Vec<_>>()
            });

            (total, data)
        });

        let status = if data.is_empty() {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::OK
        };

        (status, Json(PageData { total, data })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

/// Get all client ids of the `Workspace`
///
/// This interface returns all `Client IDs` that includes history in the `Workspace`
///
/// Every client write something into a `Workspace` will has a unique id.
///
/// For example:
///   - A user writes a new `Block` to a `Workspace` through `Client` on the front end, which will generate a series of histories.
///     A `Client ID` contained in these histories will be randomly generated by the `Client` and will remain unchanged until the Client instance is destroyed
///   - When the server initializes or get the `Workspace`, a `Client` will be created. This `Client` will not be destroyed until the server restarts.
///     Therefore, the `Client ID` in the history generated by modifying `Block` through HTTP API will remain unchanged until the server restarts.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}/history",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace history client ids", body = [u64]),
        (status = 500, description = "Failed to get workspace history")
    )
)]
pub async fn history_workspace_clients(
    Extension(context): Extension<Arc<Context>>,
    Path(ws_id): Path<String>,
) -> Response {
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        if let Some(history) = parse_history_client(&workspace.doc()) {
            Json(history).into_response()
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

/// Get the history generated by a specific `Client ID` of the `Workspace`
///
/// If client id set to 0, return all history of the `Workspace`.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace_id}/history/{client}",
    params(
        ("workspace_id", description = "workspace id"),
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
) -> Response {
    let (ws_id, client) = params;
    if let Ok(workspace) = context.storage.get_workspace(&ws_id).await {
        if let Ok(client) = client.parse::<u64>() {
            if let Some(json) = parse_history(&workspace.doc(), client)
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
        (
            StatusCode::NOT_FOUND,
            format!("Workspace({ws_id:?}) not found"),
        )
            .into_response()
    }
}

/// Register a webhook for all block changes from `workspace_id`
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/subscribe",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Subscribe workspace succeed"),
        (status = 404, description = "Workspace not found"),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn subscribe_workspace(
    Extension(context): Extension<Arc<Context>>,
    Extension(client): Extension<Arc<Client>>,
    Path(ws_id): Path<String>,
    Json(payload): Json<SubscribeWorkspace>,
) -> Response {
    info!(
        "subscribe_workspace {}, hook endpoint: {}",
        ws_id, payload.hook_endpoint
    );
    let hook_endpoint = payload.hook_endpoint;
    if let Ok(mut workspace) = context.storage.get_workspace(&ws_id).await {
        workspace.try_subscribe_all_blocks();
        if let Some(runtime) = workspace.get_tokio_runtime() {
            workspace.set_callback(Box::new(move |block_ids| {
                let ws_id = ws_id.clone();
                let client = client.clone();
                let hook_endpoint = hook_endpoint.clone();
                debug!("blocks: {:?} changed from workspace: {}", block_ids, ws_id);
                runtime.spawn(async move {
                    let response = client
                        .post(hook_endpoint)
                        .json(&WorkspaceNotify {
                            workspace_id: ws_id.to_string(),
                            block_ids,
                        })
                        .send()
                        .await;
                    match response {
                        Err(e) => error!("Failed to send notify: {}", e),
                        Ok(response) => info!(
                            "notified hook endpoint, endpoint response status: {}",
                            response.status()
                        ),
                    }
                });
            }));
            StatusCode::OK.into_response()
        } else {
            error!("get tokio runtime failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    } else {
        let err_msg = format!("Workspace: {} not found", ws_id);
        error!(err_msg);
        (StatusCode::NOT_FOUND, err_msg).into_response()
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod test {
    use super::*;

    #[tokio::test]
    async fn workspace() {
        use axum_test_helper::TestClient;

        let pool = DbPool::init_memory_pool().await.unwrap();
        let context = Arc::new(Context::new(Some(pool)).await);

        let app = super::workspace_apis(Router::new()).layer(Extension(context));

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
