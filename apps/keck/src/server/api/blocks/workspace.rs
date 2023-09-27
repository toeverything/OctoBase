use axum::{
    extract::{BodyStream, Path, Query},
    response::Response,
};
use futures::{
    future,
    stream::{iter, StreamExt},
};
use jwst_core::DocStorage;
use jwst_storage::JwstStorageError;
use utoipa::IntoParams;

use super::*;

/// Get a exists `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if `Workspace` is exists.
/// - Return 404 Not Found if `Workspace` not exists.
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
pub async fn get_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("get_workspace: {}", workspace);
    if let Ok(workspace) = context.get_workspace(&workspace).await {
        Json(workspace).into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
    }
}

/// Init a `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if init success.
/// - Return 304 Not Modified if `Workspace` is exists.
/// - Return 500 Internal Server Error if init failed.
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}/init",
    params(
        ("workspace", description = "workspace id"),
    ),
    request_body(
        content = BodyStream,
        content_type="application/octet-stream"
    ),
    responses(
        (status = 200, description = "Workspace init success"),
        (status = 304, description = "Workspace is exists"),
        (status = 500, description = "Failed to init a workspace")
    )
)]
pub async fn init_workspace(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    body: BodyStream,
) -> Response {
    info!("init_workspace: {}", workspace);

    let mut has_error = false;
    let data = body
        .take_while(|x| {
            has_error = x.is_err();
            future::ready(x.is_ok())
        })
        .filter_map(|data| future::ready(data.ok()))
        .flat_map(|buffer| iter(buffer))
        .collect::<Vec<u8>>()
        .await;

    if has_error {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    } else if let Err(e) = context.init_workspace(workspace, data).await {
        if matches!(e, JwstStorageError::WorkspaceExists(_)) {
            return StatusCode::NOT_MODIFIED.into_response();
        }
        warn!("failed to init workspace: {}", e.to_string());
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    } else {
        StatusCode::OK.into_response()
    }
}

/// Create a `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if init success or `Workspace` is
///   exists.
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
pub async fn set_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("set_workspace: {}", workspace);
    match context.create_workspace(workspace).await {
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
pub async fn delete_workspace(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response {
    info!("delete_workspace: {}", workspace);
    if context.storage.docs().delete_workspace(&workspace).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    StatusCode::NO_CONTENT.into_response()
}

/// Block search query
// See doc for using utoipa search queries example here: https://github.com/juhaku/utoipa/blob/6c7f6a2d/examples/todo-axum/src/main.rs#L124-L130
#[derive(Deserialize, IntoParams)]
pub struct BlockSearchQuery {
    /// Search by title and text.
    _query: String,
}

/// Search workspace blocks of server
///
/// This will return back a list of relevant blocks.
// #[utoipa::path(
//     get,
//     tag = "Workspace",
//     context_path = "/api/search",
//     path = "/{workspace}",
//     params(
//         ("workspace", description = "workspace id"),
//         BlockSearchQuery,
//     ),
//     responses(
//         (status = 200, description = "Search results", body = SearchResults),
//     )
// )]
// pub async fn workspace_search(
//     Extension(context): Extension<Arc<Context>>,
//     Path(workspace): Path<String>,
//     query: Query<BlockSearchQuery>,
// ) -> Response { let query_text = &query.query; let ws_id = workspace; info!("workspace_search: {ws_id:?} query =
//   {query_text:?}"); if let Ok(workspace) = context.get_workspace(&ws_id).await { match workspace.search(query_text) {
//   Ok(list) => { debug!("workspace_search: {ws_id:?} query = {query_text:?}; {list:#?}"); Json(list).into_response() }
//   Err(err) => { error!("Internal server error calling workspace_search: {err:?}");
//   StatusCode::INTERNAL_SERVER_ERROR.into_response() } } } else { (StatusCode::NOT_FOUND,
//   format!("Workspace({ws_id:?}) not found")).into_response() }
// }

// #[utoipa::path(
//     get,
//     tag = "Workspace",
//     context_path = "/api/search",
//     path = "/{workspace}/index",
//     params(
//         ("workspace", description = "workspace id"),
//     ),
//     responses(
//         (status = 200, description = "result", body = Vec<String>),
//         (status = 404, description = "Workspace not found")
//     )
// )]
// pub async fn get_search_index(Extension(context): Extension<Arc<Context>>, Path(workspace): Path<String>) -> Response
// {     info!("get_search_index: {workspace:?}");

//     if let Ok(workspace) = context.get_workspace(&workspace).await {
//         Json(workspace.metadata().search_index).into_response()
//     } else {
//         (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
//     }
// }

// #[utoipa::path(
//     post,
//     tag = "Workspace",
//     context_path = "/api/search",
//     path = "/{workspace}/index",
//     params(
//         ("workspace", description = "workspace id"),
//     ),
//     responses(
//         (status = 200, description = "success"),
//         (status = 400, description = "Bad Request"),
//         (status = 404, description = "Workspace not found")
//     )
// )]
// pub async fn set_search_index(
//     Extension(context): Extension<Arc<Context>>,
//     Path(workspace): Path<String>,
//     Json(fields): Json<Vec<String>>,
// ) -> Response { info!("set_search_index: {workspace:?} fields = {fields:?}");

//     if let Ok(workspace) = context.get_workspace(&workspace).await {
//         if let Ok(true) = workspace.set_search_index(fields) {
//             StatusCode::OK.into_response()
//         } else {
//             StatusCode::BAD_REQUEST.into_response()
//         }
//     } else {
//         (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
//     }
// }

/// Get `Block` in `Workspace`
/// - Return 200 and `Block`'s ID.
/// - Return 404 Not Found if `Workspace` or `Block` not exists.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/block",
    path = "/{workspace}/blocks",
    params(
        ("workspace", description = "workspace id"),
        Pagination
    ),
    responses(
        (status = 200, description = "Get Blocks", body = PageData<[Block]>),
        (status = 404, description = "Workspace or block not found"),
    )
)]
pub async fn get_workspace_block(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    Query(pagination): Query<Pagination>,
) -> Response {
    let Pagination { offset, limit } = pagination;
    info!("get_workspace_block: {workspace:?}");
    if let Ok(space) = context
        .get_workspace(&workspace)
        .await
        .and_then(|mut ws| Ok(ws.get_blocks()?))
    {
        let total = space.block_count() as usize;
        let data = space.blocks(|blocks| blocks.skip(offset).take(limit).collect::<Vec<_>>());

        let status = if data.is_empty() {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::OK
        };

        (status, Json(PageData { total, data })).into_response()
    } else {
        (StatusCode::NOT_FOUND, format!("Workspace({workspace:?}) not found")).into_response()
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
        assert_eq!(resp.json::<schema::Workspace>().await, schema::Workspace::default());

        let resp = client.get("/block/test").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.json::<schema::Workspace>().await, schema::Workspace::default());
    }
}
