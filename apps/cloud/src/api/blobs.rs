use crate::{context::Context, error_status::ErrorStatus};
use axum::{
    body::StreamBody,
    extract::{BodyStream, Path},
    headers::ContentLength,
    response::{IntoResponse, Response},
    Extension, Json, TypedHeader,
};
use chrono::{DateTime, Utc};
use cloud_database::Claims;
use futures::{future, StreamExt};
use http::{
    header::{
        CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH,
        LAST_MODIFIED,
    },
    HeaderMap, HeaderValue,
};
use jwst::{error, BlobStorage};
use mime::APPLICATION_OCTET_STREAM;
use std::sync::Arc;

impl Context {
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        method: http::Method,
        headers: HeaderMap,
    ) -> Response {
        if let Some(etag) = headers.get(IF_NONE_MATCH).and_then(|h| h.to_str().ok()) {
            if etag == id {
                return ErrorStatus::NotModify.into_response();
            }
        }

        let Ok(meta) = self.storage.blobs().get_metadata(workspace.clone(), id.clone()).await else {
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
            HeaderValue::from_static(APPLICATION_OCTET_STREAM.essence_str()),
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

        if method == http::Method::HEAD {
            return header.into_response();
        };

        let Ok(file) = self.storage.blobs().get_blob(workspace, id).await else {
            return ErrorStatus::NotFound.into_response();
        };

        (header, StreamBody::new(file)).into_response()
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

    async fn upload_workspace(&self, stream: BodyStream) -> Vec<u8> {
        let mut has_error = false;
        let stream = stream
            .take_while(|x| {
                has_error = x.is_err();
                future::ready(x.is_ok())
            })
            .filter_map(|data| future::ready(data.ok()));
        let mut stream = Box::pin(stream);
        let mut res = vec![];
        while let Some(b) = stream.next().await {
            let mut chunk = b.to_vec();
            res.append(&mut chunk);
        }
        res
    }
}

///  Get `blob`.
/// - Return `blob`.
#[utoipa::path(
    get,
    tag = "Blob",
    context_path = "/api/blob",
    path = "/{name}",
    params(
        ("name", description = "hash of blob"),
    )
)]
pub async fn get_blob(
    Extension(ctx): Extension<Arc<Context>>,
    Path(id): Path<String>,
    method: http::Method,
    headers: HeaderMap,
) -> Response {
    ctx.get_blob(None, id, method, headers).await
}

///  Upload `blob`.
/// - Return `hash`.
#[utoipa::path(put, tag = "Blob", context_path = "/api", path = "/blob")]
pub async fn upload_blob(
    Extension(ctx): Extension<Arc<Context>>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    if length.0 > 10 * 1024 * 1024 {
        return ErrorStatus::PayloadTooLarge.into_response();
    }

    ctx.upload_blob(stream, None).await
}

///  Get `blob` by workspace_id and hash.
/// - Return `blob`.
#[utoipa::path(
    get,
    tag = "Blob",
    context_path = "/api/workspace",
    path = "/{workspace_id}/blob/{name}",
    params(
        ("workspace_id", description = "id of workspace"),
        ("name", description = "hash of blob"),
    )
)]
pub async fn get_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    // Extension(claims): Extension<Arc<Claims>>,
    Path((workspace_id, id)): Path<(String, String)>,
    method: http::Method,
    headers: HeaderMap,
) -> Response {
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
/// - Return `hash`.
#[utoipa::path(
    put,
    tag = "Blob",
    context_path = "/api/workspace",
    path = "/{workspace_id}/blob",
    params(
        ("workspace_id", description = "id of workspace"),
    )
)]
pub async fn upload_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
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

/// Create `Workspace` .
/// - Return  `Workspace`'s data.
#[utoipa::path(post, tag = "Workspace", context_path = "/api", path = "/workspace")]
pub async fn create_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    TypedHeader(_length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    match ctx.db.create_normal_workspace(claims.user.id.clone()).await {
        Ok(data) => {
            let id = data.id.to_string();
            let update = ctx.upload_workspace(stream).await;
            if !ctx.storage.full_migrate(id, Some(update), true).await {
                return ErrorStatus::InternalServerError.into_response();
            }
            ctx.user_channel
                .add_user_observe(claims.user.id.clone(), ctx.clone())
                .await;
            Json(data).into_response()
        }
        Err(e) => {
            error!("Failed to create workspace: {}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}
