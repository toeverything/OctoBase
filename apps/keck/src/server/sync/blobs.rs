use super::*;
use axum::{
    extract::{BodyStream, Path},
    headers::ContentLength,
    http::{
        header::{
            CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH,
            LAST_MODIFIED,
        },
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    Json, TypedHeader,
};
use futures::{future, StreamExt};
use jwst::BlobStorage;
use jwst_rpc::RpcContextImpl;
use std::sync::Arc;
use time::{format_description::well_known::Rfc2822, OffsetDateTime};

#[derive(Serialize)]
struct BlobStatus {
    exists: bool,
    id: String,
}

impl Context {
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        method: Method,
        headers: HeaderMap,
    ) -> Response {
        if let Some(etag) = headers.get(IF_NONE_MATCH).and_then(|h| h.to_str().ok()) {
            if etag == id {
                return StatusCode::NOT_MODIFIED.into_response();
            }
        }

        let Ok(meta) = self.get_storage().blobs().get_metadata(workspace.clone(), id.clone(), None).await else {
            return StatusCode::NOT_FOUND.into_response()
        };

        if let Some(modified_since) = headers
            .get(IF_MODIFIED_SINCE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| OffsetDateTime::parse(s, &Rfc2822).ok())
        {
            if meta.last_modified.timestamp() <= modified_since.unix_timestamp() {
                return StatusCode::NOT_MODIFIED.into_response();
            }
        }

        let mut header = HeaderMap::with_capacity(5);
        header.insert(ETAG, HeaderValue::from_str(&id).unwrap());
        header.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        header.insert(
            LAST_MODIFIED,
            HeaderValue::from_str(
                &OffsetDateTime::from_unix_timestamp(meta.last_modified.timestamp())
                    .unwrap()
                    .format(&Rfc2822)
                    .unwrap(),
            )
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

        let Ok(file) = self.get_storage().blobs().get_blob(workspace, id, None).await else {
            return StatusCode::NOT_FOUND.into_response()
        };

        if meta.size != file.len() as i64 {
            header.insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&file.len().to_string()).unwrap(),
            );
        }

        (header, file).into_response()
    }

    async fn upload_blob(&self, stream: BodyStream, workspace: Option<String>) -> Response {
        // TODO: cancel
        let mut has_error = false;
        let stream = stream
            .take_while(|x| {
                has_error = x.is_err();
                future::ready(x.is_ok())
            })
            .filter_map(|data| future::ready(data.ok()));

        if let Ok(id) = self
            .get_storage()
            .blobs()
            .put_blob_stream(workspace.clone(), stream)
            .await
        {
            if has_error {
                let _ = self.get_storage().blobs().delete_blob(workspace, id).await;
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            } else {
                Json(BlobStatus { id, exists: true }).into_response()
            }
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn get_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Path((workspace_id, id)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    ctx.get_blob(Some(workspace_id), id, method, headers).await
}

pub async fn upload_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace_id): Path<String>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    if length.0 > 10 * 1024 * 1024 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }

    ctx.upload_blob(stream, Some(workspace_id)).await
}
