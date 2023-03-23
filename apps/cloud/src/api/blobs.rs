use crate::{context::Context, error_status::ErrorStatus};
use axum::{
    extract::{BodyStream, Path},
    headers::ContentLength,
    http::{
        header::{
            CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH,
            LAST_MODIFIED,
        },
        HeaderMap, HeaderValue, Method,
    },
    response::{IntoResponse, Response},
    Extension, TypedHeader,
};
use chrono::{DateTime, Utc};
use cloud_database::Claims;
use futures::{future, StreamExt};
use jwst::{error, BlobStorage};
use jwst_logger::{info, instrument, tracing};
use mime::APPLICATION_OCTET_STREAM;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

impl Context {
    #[instrument(skip(self, method, headers))]
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        method: Method,
        headers: HeaderMap,
    ) -> Response {
        info!("get_blob enter");

        let (id, params) = {
            let path = PathBuf::from(id.clone());
            let ext = path
                .extension()
                .and_then(|s| s.to_str().map(|s| s.to_string()));
            let id = path
                .file_stem()
                .and_then(|s| s.to_str().map(|s| s.to_string()))
                .unwrap_or(id);

            (id, ext.map(|ext| HashMap::from([("format".into(), ext)])))
        };

        if let Some(etag) = headers.get(IF_NONE_MATCH).and_then(|h| h.to_str().ok()) {
            if etag == id {
                return ErrorStatus::NotModify.into_response();
            }
        }

        let Ok(meta) = self.storage.blobs().get_metadata(workspace.clone(), id.clone(), params.clone()).await else {
            return ErrorStatus::NotFound.into_response();
        };

        if let Some(modified_since) = headers
            .get(IF_MODIFIED_SINCE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok())
        {
            if meta.last_modified <= modified_since.naive_utc() {
                return ErrorStatus::NotModify.into_response();
            }
        }

        let mut header = HeaderMap::with_capacity(5);
        header.insert(ETAG, HeaderValue::from_str(&id).unwrap());
        header.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(&meta.content_type).unwrap_or(HeaderValue::from_static(
                APPLICATION_OCTET_STREAM.essence_str(),
            )),
        );
        header.insert(
            LAST_MODIFIED,
            HeaderValue::from_str(&DateTime::<Utc>::from_utc(meta.last_modified, Utc).to_rfc2822())
                .unwrap(),
        );
        header.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&meta.size.to_string()).unwrap(),
        );
        header.insert(
            CACHE_CONTROL,
            HeaderValue::from_str("public, immutable, max-age=31536000").unwrap(),
        );

        if method == Method::HEAD {
            return header.into_response();
        };

        let Ok(file) = self.storage.blobs().get_blob(workspace, id, params.clone()).await else {
            return ErrorStatus::NotFound.into_response();
        };

        if meta.size != file.len() as u64 {
            header.insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&file.len().to_string()).unwrap(),
            );

            if let Some(params) = params {
                if let Some(format) = params.get("format") {
                    header.insert(
                        CONTENT_TYPE,
                        HeaderValue::from_str(&format!("image/{format}")).unwrap_or(
                            HeaderValue::from_static(APPLICATION_OCTET_STREAM.essence_str()),
                        ),
                    );
                }
            }
        }

        (header, file).into_response()
    }

    #[instrument(skip(self, stream))]
    async fn upload_blob(&self, stream: BodyStream, workspace: Option<String>) -> Response {
        info!("upload_blob enter");
        // TODO: cancel
        let mut has_error = false;
        let stream = stream
            .take_while(|x| {
                has_error = x.is_err();
                future::ready(x.is_ok())
            })
            .filter_map(|data| future::ready(data.ok()));

        if let Ok(id) = self
            .storage
            .blobs()
            .put_blob(workspace.clone(), stream)
            .await
        {
            if has_error {
                let _ = self.storage.blobs().delete_blob(workspace, id).await;
                ErrorStatus::InternalServerError.into_response()
            } else {
                id.into_response()
            }
        } else {
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

///  Get `blob`.
/// - Return 200 ok and `blob`.
/// - Return 304 the file is not modified.
/// - Return 404 the file does not exist.
#[utoipa::path(
    get,
    tag = "Blob",
    context_path = "/api/blob",
    path = "/{name}",
    params(
        ("name", description = "hash of blob"),
    ),
    responses(
        (status = 200, description = "Successfully get blob",body=BodyStream),
        (status = 304, description = "The file is not modified"),
        (status = 404, description = "The file does not exist"),
    )
)]
#[instrument(skip(ctx, method, headers))]
pub async fn get_blob(
    Extension(ctx): Extension<Arc<Context>>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    info!("get_blob enter");
    ctx.get_blob(None, id, method, headers).await
}

///  Upload `blob`.
/// - Return 200 and `hash`.
/// - Return 413 upload file size exceeds 10MB.
#[utoipa::path(put, tag = "Blob", context_path = "/api", path = "/blob",
request_body(content=BodyStream, description="file size needs to be less than 10MB", content_type="application/octet-stream"),
    responses(
        (status = 200, description = "Successfully upload blob",body=String),
        (status = 413, description = "Upload file size exceeds 10MB"),
    ))]
#[instrument(skip(ctx, length, stream))]
pub async fn upload_blob(
    Extension(ctx): Extension<Arc<Context>>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    info!("upload_blob enter");
    if length.0 > 10 * 1024 * 1024 {
        return ErrorStatus::PayloadTooLarge.into_response();
    }

    ctx.upload_blob(stream, None).await
}

///  Get `blob` by workspace_id and hash.
/// - Return 200 and `blob`.
/// - Return 304 the file is not modified.
/// - Return 404 the file or workspace does not exist.
#[utoipa::path(
    get,
    tag = "Blob",
    context_path = "/api/workspace",
    path = "/{workspace_id}/blob/{name}",
    params(
        ("workspace_id", description = "id of workspace"),
        ("name", description = "hash of blob"),
    ),
    responses(
        (status = 200, description = "Successfully get blob",body=BodyStream),
        (status = 304, description = "The file is not modified"),
        (status = 404, description = "The file or workspace does not exist"),
    )
)]
#[instrument(skip(ctx, method, headers))]
pub async fn get_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    // Extension(claims): Extension<Arc<Claims>>,
    Path((workspace_id, id)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    info!("get_blob_in_workspace enter");
    // match ctx
    //     .db
    //     .can_read_workspace(claims.user.id.clone(), workspace_id.clone())
    //     .await
    // {
    //     Ok(true) => (),
    //     Ok(false) => return ErrorStatus::Forbidden.into_response(),
    //     Err(_) => return ErrorStatus::InternalServerError.into_response(),
    // }

    ctx.get_blob(Some(workspace_id), id, method, headers).await
}

///  Upload `blob` by workspace_id.
/// - Return 200 and `hash`.
/// - Return 403 sorry, you do not have permission.
/// - Return 404 the workspace does not exist.
/// - Return 413 upload file size exceeds 10MB.
/// - Return 500 internal server error.
#[utoipa::path(
    put,
    tag = "Blob",
    context_path = "/api/workspace",
    path = "/{workspace_id}/blob",
    params(
        ("workspace_id", description = "id of workspace"),
    ),
    request_body(content=BodyStream, description="file size needs to be less than 10MB", content_type="application/octet-stream"),
    responses(
        (status = 200, description = "Successfully upload blob",body=String),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 404, description = "The workspace does not exist"),
        (status = 413, description = "Upload file size exceeds 10MB"),
        (status = 500, description = "Internal server error"),
    )
)]
#[instrument(skip(ctx, claims, length, stream), fields(user_id = %claims.user.id))]
pub async fn upload_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    info!("upload_blob_in_workspace enter");
    if length.0 > 10 * 1024 * 1024 {
        return ErrorStatus::PayloadTooLarge.into_response();
    }

    match ctx
        .db
        .can_read_workspace(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(true) => (),
        Ok(false) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to check read workspace: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    ctx.upload_blob(stream, Some(workspace_id)).await
}

#[cfg(test)]
mod test {
    use axum::{body::Body, http::StatusCode};
    use axum_test_helper::TestClient;
    use bytes::Bytes;
    use cloud_database::CloudDatabase;
    use futures::stream;

    use super::{
        super::{make_rest_route, Context},
        *,
    };

    #[tokio::test]
    async fn test_upload_blob() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(
            test_data
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );
        let body_stream = Body::wrap_stream(test_data_stream);

        let resp = client
            .put("/blob")
            .header("Content-Length", test_data_len.to_string())
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes_per_chunk = 1024;
        let num_chunks = 10 * 1024 + 1;
        let large_stream = stream::repeat(Bytes::from(vec![0; bytes_per_chunk]))
            .take(num_chunks)
            .map(Ok::<_, std::io::Error>);

        let body_stream = Body::wrap_stream(large_stream);
        let content_length = (bytes_per_chunk * num_chunks).to_string();
        let resp = client
            .put("/blob")
            .header("Content-Length", content_length)
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn test_get_blob() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(
            test_data
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );
        let body_stream = Body::wrap_stream(test_data_stream);

        let resp = client
            .put("/blob")
            .header("Content-Length", test_data_len.to_string())
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let blob_name = resp.text().await;
        let url = format!("/blob/{}", blob_name);
        let resp = client.get(&url).send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client.get("/blob/mock_id").send().await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
