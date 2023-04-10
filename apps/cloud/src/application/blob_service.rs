use crate::{
    context::Context, infrastructure::auth::get_claim_from_headers,
    infrastructure::error_status::ErrorStatus,
};
use axum::{
    extract::BodyStream,
    headers::ContentLength,
    http::{
        header::{
            CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH,
            LAST_MODIFIED,
        },
        HeaderMap, HeaderValue, Method,
    },
    Json,
};
use chrono::{DateTime, Utc};
use futures::{future, StreamExt};
use jwst::error;
use jwst::BlobStorage;
use jwst_logger::{info, instrument, tracing};
use mime::APPLICATION_OCTET_STREAM;
use serde::Serialize;
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};

#[derive(Serialize)]
pub struct Usage {
    pub blob_usage: BlobUsage,
}

#[derive(Serialize)]
pub struct BlobUsage {
    pub usage: u64,
    pub max_usage: u64,
}

const MAX_USAGE: u64 = 10 * 1024 * 1024 * 1024;
const MAX_BLOB_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct CloudBlobService {}

impl CloudBlobService {
    pub fn new() -> Self {
        Self {}
    }

    #[instrument(skip(ctx, method, headers))]
    pub async fn get_blob_in_workspace(
        &self,
        ctx: &Arc<Context>,
        workspace_id: String,
        id: String,
        method: Method,
        headers: HeaderMap,
    ) -> Result<(HeaderMap, Option<Vec<u8>>), ErrorStatus> {
        info!("get_blob_in_workspace enter");
        match ctx.db.is_public_workspace(workspace_id.clone()).await {
            Ok(true) => (),
            Ok(false) => match get_claim_from_headers(&headers, &ctx.key.jwt_decode) {
                Some(claims) => {
                    match ctx
                        .db
                        .can_read_workspace(claims.user.id.clone(), workspace_id.clone())
                        .await
                    {
                        Ok(true) => (),
                        Ok(false) => return Err(ErrorStatus::Forbidden),
                        Err(_) => return Err(ErrorStatus::InternalServerError),
                    }
                }
                None => return Err(ErrorStatus::Forbidden),
            },
            Err(_) => return Err(ErrorStatus::InternalServerError),
        }

        self.get_blob(&ctx, Some(workspace_id), id, method, headers)
            .await
    }

    #[instrument(skip(ctx, length, stream))]
    pub async fn upload_blob_in_workspace(
        &self,
        ctx: &Arc<Context>,
        user_id: String,
        workspace_id: String,
        length: ContentLength,
        stream: BodyStream,
    ) -> Result<String, ErrorStatus> {
        info!("upload_blob_in_workspace enter");
        if length.0 > MAX_BLOB_SIZE {
            return Err(ErrorStatus::PayloadTooLarge);
        }

        let Ok(usage) = self.get_user_resource_usage(&ctx, user_id.clone()).await else {
            return Err(ErrorStatus::InternalServerError);
        };
        if usage.blob_usage.usage + length.0 > usage.blob_usage.max_usage {
            return Err(ErrorStatus::PayloadExceedsLimit("10GB".to_string()));
        }

        match ctx
            .db
            .can_read_workspace(user_id.clone(), workspace_id.clone())
            .await
        {
            Ok(true) => (),
            Ok(false) => return Err(ErrorStatus::Forbidden),
            Err(e) => {
                error!("Failed to check read workspace: {}", e);
                return Err(ErrorStatus::InternalServerError);
            }
        }

        self.upload_blob(&ctx, stream, Some(workspace_id)).await
    }

    #[instrument(skip(ctx))]
    pub async fn get_user_resource_usage(
        &self,
        ctx: &Arc<Context>,
        user_id: String,
    ) -> Result<Json<Usage>, ErrorStatus> {
        info!("get_user_resource enter");
        let Ok(workspace_id_list) = ctx.db.get_user_owner_workspaces(user_id).await else {
            return Err(ErrorStatus::InternalServerError);
        };
        let mut total_size = 0;
        for workspace_id in workspace_id_list {
            let size = ctx
                .storage
                .blobs()
                .get_blobs_size(workspace_id.to_string())
                .await
                .map_err(|_| ErrorStatus::InternalServerError)?;
            total_size += size as u64;
        }
        let blob_usage = BlobUsage {
            usage: total_size,
            max_usage: MAX_USAGE,
        };
        let usage = Usage { blob_usage };
        Ok(Json(usage))
    }

    #[instrument(skip(ctx, method, headers))]
    async fn get_blob(
        &self,
        ctx: &Arc<Context>,
        workspace: Option<String>,
        id: String,
        method: Method,
        headers: HeaderMap,
    ) -> Result<(HeaderMap, Option<Vec<u8>>), ErrorStatus> {
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
                return Err(ErrorStatus::NotModify);
            }
        }

        let Ok(meta) = ctx.storage.blobs().get_metadata(workspace.clone(), id.clone(), params.clone()).await else {
            return Err(ErrorStatus::NotFound);
        };

        if let Some(modified_since) = headers
            .get(IF_MODIFIED_SINCE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok())
        {
            if meta.last_modified <= modified_since.naive_utc() {
                return Err(ErrorStatus::NotModify);
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
            return Ok((header, None));
        };

        let Ok(file) = ctx.storage.blobs().get_blob(workspace, id, params.clone()).await else {
            return Err(ErrorStatus::NotFound);
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

        Ok((header, Some(file)))
    }

    #[instrument(skip(ctx, stream))]
    async fn upload_blob(
        &self,
        ctx: &Arc<Context>,
        stream: BodyStream,
        workspace: Option<String>,
    ) -> Result<String, ErrorStatus> {
        info!("upload_blob enter");
        // TODO: cancel
        let mut has_error = false;
        let stream = stream
            .take_while(|x| {
                has_error = x.is_err();
                future::ready(x.is_ok())
            })
            .filter_map(|data| future::ready(data.ok()));

        if let Ok(id) = ctx
            .storage
            .blobs()
            .put_blob(workspace.clone(), stream)
            .await
        {
            if has_error {
                let _ = ctx.storage.blobs().delete_blob(workspace, id).await;
                Err(ErrorStatus::InternalServerError)
            } else {
                Ok(id)
            }
        } else {
            Err(ErrorStatus::InternalServerError)
        }
    }
}
