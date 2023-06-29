use super::*;
use axum::{extract::BodyStream, response::Response, routing::post};
use futures::{future, StreamExt};
use jwst::BlobStorage;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct BlobStatus {
    id: Option<String>,
    exists: bool,
}

/// Check a `Blob` is exists by id
/// - Return 200 if `Blob` is exists.
/// - Return 404 Not Found if `Workspace` or `Blob` not exists.
#[utoipa::path(
    head,
    tag = "Blobs",
    context_path = "/api/blobs",
    path = "/{workspace_id}/{hash}",
    params(
        ("workspace_id", description = "workspace id"),
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
    path = "/{workspace_id}/{hash}",
    params(
        ("workspace_id", description = "workspace id"),
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
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
        ("hash", description = "blob hash"),
    ),
    request_body(
        content = BodyStream,
        content_type="application/octet-stream"
    ),
    responses(
        (status = 200, description = "Blob was saved", body = BlobStatus),
        (status = 404, description = "Workspace not found", body = BlobStatus),
    )
)]
pub async fn set_blob(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    body: BodyStream,
) -> Response {
    info!("set_blob: {}", workspace);

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
        .put_blob_stream(Some(workspace.clone()), body)
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
            Json(BlobStatus {
                id: Some(id),
                exists: true,
            })
            .into_response()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(BlobStatus {
                id: None,
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
    path = "/{workspace_id}/{hash}",
    params(
        ("workspace_id", description = "workspace id"),
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
    router.route("/blobs/:workspace", post(set_blob)).route(
        "/blobs/:workspace/:blob",
        head(check_blob).get(get_blob).delete(delete_blob),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, Bytes};
    use axum_test_helper::TestClient;
    use futures::stream;
    use jwst_storage::BlobStorageType;

    #[tokio::test]
    async fn test_blobs_apis() {
        let ctx = Context::new(
            JwstStorage::new_with_migration("sqlite::memory:", BlobStorageType::DB)
                .await
                .ok(),
            None,
        )
        .await;
        let client = TestClient::new(blobs_apis(Router::new()).layer(Extension(Arc::new(ctx))));

        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(
            test_data
                .clone()
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );

        //upload blob in workspace
        let resp = client
            .post("/blobs/test")
            .header("Content-Length", test_data_len.clone().to_string())
            .body(Body::wrap_stream(test_data_stream.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let hash = resp_json["id"].as_str().unwrap().to_string();

        let resp = client.head(&format!("/blobs/test/{}", hash)).send().await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client.get(&format!("/blobs/test/{}", hash)).send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(test_data, resp.bytes().await.to_vec());

        let resp = client.delete(&format!("/blobs/test/{}", hash)).send().await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let resp = client.delete(&format!("/blobs/test/{}", hash)).send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = client.head(&format!("/blobs/test/{}", hash)).send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = client.get(&format!("/blobs/test/{}", hash)).send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = client.get("/blobs/test/not_exists_id").send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
