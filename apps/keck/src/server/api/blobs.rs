use super::*;

use axum::{extract::BodyStream, response::Response};
use futures::{future, StreamExt};
use jwst::BlobStorage;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
struct BlobStatus {
    id: String,
    exists: bool,
}

/// Check a `Blob` is exists by id
/// - Return 200 if `Blob` is exists.
/// - Return 404 Not Found if `Workspace` or `Blob` not exists.
#[utoipa::path(
    head,
    tag = "Blobs",
    context_path = "/api/blobs",
    path = "/{workspace}/{hash}",
    params(
        ("workspace", description = "workspace id"),
        ("hash", description = "blob hash"),
    ),
    responses(
        (status = 200, description = "Blob exists"),
        (status = 404, description = "Workspace or blob content not found"),
        (status = 500, description = "Failed to query blobs"),
    )
)]
pub async fn check_blob(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> Response {
    let (workspace, hash) = params;
    info!("check_blob: {}, {}", workspace, hash);
    if let Ok(exists) = context
        .storage
        .blobs()
        .check_blob(Some(workspace), hash)
        .await
    {
        if exists {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        }
        .into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

/// Get a `Blob` by hash
/// - Return 200 and `Blob` data if `Blob` is exists.
/// - Return 404 Not Found if `Workspace` or `Blob` not exists.
#[utoipa::path(
    get,
    tag = "Blobs",
    context_path = "/api/blobs",
    path = "/{workspace}/{hash}",
    params(
        ("workspace", description = "workspace id"),
        ("hash", description = "blob hash"),
    ),
    responses(
        (status = 200, description = "Get blob", body = Vec<u8>),
        (status = 404, description = "Workspace or blob content not found"),
    )
)]
pub async fn get_blob(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> Response {
    let (workspace, hash) = params;
    info!("get_blob: {}, {}", workspace, hash);
    if let Ok(blob) = context
        .storage
        .blobs()
        .get_blob(Some(workspace), hash, None)
        .await
    {
        blob.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Save `Blob` if not exists
/// - Return 200 if `Blob` save successful.
/// - Return 404 Not Found if `Workspace` not exists.
#[utoipa::path(
    post,
    tag = "Blobs",
    context_path = "/api/blobs",
    path = "/{workspace}/{hash}",
    params(
        ("workspace", description = "workspace id"),
        ("hash", description = "blob hash"),
    ),
    request_body(
        content = Vec<u8>,
    ),
    responses(
        (status = 200, description = "Blob was saved", body = BlobStatus),
        (status = 404, description = "Workspace not found", body = BlobStatus),
    )
)]
pub async fn set_blob(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
    body: BodyStream,
) -> Response {
    let (workspace, hash) = params;
    info!("set_blob: {}, {}", workspace, hash);

    let mut has_error = false;
    let body = body
        .take_while(|x| {
            has_error = x.is_err();
            future::ready(x.is_ok())
        })
        .filter_map(|data| future::ready(data.ok()));

    if let Ok(id) = context
        .storage
        .blobs()
        .put_blob(Some(workspace.clone()), body)
        .await
    {
        if has_error {
            let _ = context
                .storage
                .blobs()
                .delete_blob(Some(workspace), id)
                .await;
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        } else {
            Json(BlobStatus { id, exists: true }).into_response()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(BlobStatus {
                id: hash,
                exists: false,
            }),
        )
            .into_response()
    }
}

/// Delete `blob` if exists
/// - Return 204 if `Blob` delete successful.
/// - Return 404 Not Found if `Workspace` or `Blob` not exists.
#[utoipa::path(
    delete,
    tag = "Blobs",
    context_path = "/api/blobs",
    path = "/{workspace}/{hash}",
    params(
        ("workspace", description = "workspace id"),
        ("hash", description = "blob hash"),
    ),
    responses(
        (status = 204, description = "Blob was deleted"),
        (status = 404, description = "Workspace or blob not found"),
    )
)]
pub async fn delete_blob(
    Extension(context): Extension<Arc<Context>>,
    Path(params): Path<(String, String)>,
) -> Response {
    let (workspace, hash) = params;
    info!("delete_blob: {}, {}", workspace, hash);

    if let Ok(success) = context
        .storage
        .blobs()
        .delete_blob(Some(workspace), hash)
        .await
    {
        if success {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        }
        .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

pub fn blobs_apis(router: Router) -> Router {
    router.route(
        "/blobs/:workspace/:blob",
        head(check_blob)
            .get(get_blob)
            .post(set_blob)
            .delete(delete_blob),
    )
}
