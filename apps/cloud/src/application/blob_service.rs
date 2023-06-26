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
pub struct BlobService {}

impl BlobService {
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
                        Err(e) => {
                            return {
                                error!("Internal server error: {:?}", e);
                                Err(ErrorStatus::InternalServerError)
                            }
                        }
                    }
                }
                None => return Err(ErrorStatus::Forbidden),
            },
            Err(e) => {
                return {
                    error!("Internal server error: {:?}", e);
                    Err(ErrorStatus::InternalServerError)
                }
            }
        }

        self.get_blob(ctx, Some(workspace_id), id, method, headers)
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

        let Ok(usage) = self.get_user_resource_usage(ctx, user_id.clone()).await else {
            error!("Failed to get user resource usage");
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

        self.upload_blob(ctx, stream, Some(workspace_id)).await
    }

    #[instrument(skip(ctx))]
    pub async fn get_user_resource_usage(
        &self,
        ctx: &Arc<Context>,
        user_id: String,
    ) -> Result<Json<Usage>, ErrorStatus> {
        info!("get_user_resource enter");
        let workspace_id_list = match ctx.db.get_user_owner_workspaces(user_id).await {
            Ok(workspace_id_list) => workspace_id_list,
            Err(e) => {
                error!("Failed to get user owner workspaces: {}", e);
                return Err(ErrorStatus::InternalServerError);
            }
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

        if meta.size != file.len() as i64 {
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

        let id = match ctx
            .storage
            .blobs()
            .put_blob_stream(workspace.clone(), stream)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to upload blob: {}", e);
                return Err(ErrorStatus::InternalServerError);
            }
        };

        if has_error {
            match ctx.storage.blobs().delete_blob(workspace, id).await {
                Ok(success) => success,
                Err(e) => {
                    error!("Failed to delete blob: {}", e);
                    return Err(ErrorStatus::InternalServerError);
                }
            };
            Err(ErrorStatus::InternalServerError)
        } else {
            Ok(id)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::infrastructure::auth::get_claim_from_token;
    use crate::{api::make_rest_route, context::Context};
    use axum::extract::BodyStream;
    use axum::extract::FromRequest;
    use axum::http::Request;
    use axum::{body::Body, http::StatusCode, Extension};
    use axum_test_helper::TestClient;
    use bytes::Bytes;
    use cloud_database::CloudDatabase;
    use futures::stream;
    use serde_json::json;

    async fn test_init() -> Arc<Context> {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        Arc::new(context)
    }

    async fn test_user_login(client: &TestClient) -> String {
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "my_username",
            "avatar_url": "my_avatar_url",
            "email": "my_email",
            "password": "my_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        // login user
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "my_email",
            "password": "my_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let access_token = resp_json["token"].as_str().unwrap().to_string();
        access_token
    }

    fn test_user_id(access_token: String, ctx: &Arc<Context>) -> String {
        let user = match get_claim_from_token(&access_token, &ctx.key.jwt_decode) {
            Some(claims) => Some(claims.user.id),
            None => None,
        };
        let user_id = user.unwrap_or("".to_string());
        assert_ne!(user_id, "");
        user_id
    }

    async fn test_workspace(client: &TestClient, access_token: String) -> String {
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(
            test_data
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );
        let body_stream = Body::wrap_stream(test_data_stream.clone());
        // create workspace
        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.clone().to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let workspace_id = resp_json["id"].as_str().unwrap().to_string();
        workspace_id
    }

    async fn test_body_stream() -> BodyStream {
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(
            test_data
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );

        // test too large
        let res = Request::builder()
            .header("content-length", test_data_len.to_string())
            .body(Body::wrap_stream(test_data_stream.clone()))
            .unwrap();
        let body_stream = BodyStream::from_request(res, &"test".to_string())
            .await
            .unwrap();
        body_stream
    }

    #[tokio::test]
    async fn test_get_blob_in_workspace() {
        // init data
        let ctx = test_init().await;
        let blob_service = &ctx.blob_service;
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));
        let client = TestClient::new(app);
        let access_token = test_user_login(&client).await;
        let user_id = test_user_id(access_token.clone(), &ctx);
        let workspace_id = test_workspace(&client, access_token.clone()).await;

        // upload blob
        let body_stream = test_body_stream().await;
        let upload_blob = blob_service
            .upload_blob_in_workspace(
                &ctx,
                user_id.clone(),
                workspace_id.clone(),
                ContentLength(5 * 1024 as u64),
                body_stream,
            )
            .await;
        assert_eq!(upload_blob.is_ok(), true);

        // test error
        let mut header_map = HeaderMap::new();
        header_map.insert(
            "authorization",
            HeaderValue::from_str(&format!("{}", access_token.clone())).unwrap(),
        );
        let err_test = blob_service
            .get_blob_in_workspace(
                &ctx,
                workspace_id.clone(),
                "other_blob".to_string(),
                Method::GET,
                header_map,
            )
            .await;
        assert_eq!(err_test.is_err(), true);

        // test success
        let mut header_map = HeaderMap::new();
        header_map.insert(
            "authorization",
            HeaderValue::from_str(&format!("{}", access_token.clone())).unwrap(),
        );
        let test_success = blob_service
            .get_blob_in_workspace(
                &ctx,
                workspace_id.clone(),
                upload_blob.ok().unwrap(),
                Method::GET,
                header_map,
            )
            .await;
        assert_eq!(test_success.is_ok(), true);
    }

    #[tokio::test]
    async fn test_upload_blob_in_workspace() {
        // init data
        let ctx = test_init().await;
        let blob_service = &ctx.blob_service;
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));
        let client = TestClient::new(app);
        let access_token = test_user_login(&client).await;
        let user_id = test_user_id(access_token.clone(), &ctx);
        let workspace_id = test_workspace(&client, access_token.clone()).await;

        // test too large
        let body_stream = test_body_stream().await;
        let too_large = blob_service
            .upload_blob_in_workspace(
                &ctx,
                user_id.clone(),
                workspace_id.clone(),
                ContentLength(20 * 1024 * 1024 as u64),
                body_stream,
            )
            .await;
        assert_eq!(too_large.is_err(), true);

        // test too large
        let body_stream = test_body_stream().await;
        let too_large = blob_service
            .upload_blob_in_workspace(
                &ctx,
                user_id.clone(),
                workspace_id.clone(),
                ContentLength(20 * 1024 * 1024 as u64),
                body_stream,
            )
            .await;
        assert_eq!(too_large.is_err(), true);

        // test try other workspace
        let body_stream = test_body_stream().await;
        let try_other_workspace = blob_service
            .upload_blob_in_workspace(
                &ctx,
                user_id.clone(),
                "other_workspace_id".to_string(),
                ContentLength(5 * 1024 as u64),
                body_stream,
            )
            .await;
        assert_eq!(try_other_workspace.is_err(), true);

        // test upload success
        let body_stream = test_body_stream().await;
        let upload = blob_service
            .upload_blob_in_workspace(
                &ctx,
                user_id.clone(),
                workspace_id.clone(),
                ContentLength(5 * 1024 as u64),
                body_stream,
            )
            .await;
        assert_eq!(upload.is_ok(), true);
    }

    #[tokio::test]
    async fn test_get_user_resource_usage() {
        // init data
        let ctx = test_init().await;
        let blob_service = &ctx.blob_service;
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));
        let client = TestClient::new(app);
        let access_token = test_user_login(&client).await;
        let user_id = test_user_id(access_token.clone(), &ctx);
        let workspace_id = test_workspace(&client, access_token.clone()).await;

        let usage = blob_service
            .get_user_resource_usage(&ctx, user_id.clone())
            .await;
        assert_eq!(usage.is_ok(), true);
        assert_eq!(usage.ok().unwrap().blob_usage.usage, 0 as u64);

        // upload blob
        let body_stream = test_body_stream().await;
        let upload = blob_service
            .upload_blob_in_workspace(
                &ctx,
                user_id.clone(),
                workspace_id.clone(),
                ContentLength(5 * 1024 as u64),
                body_stream,
            )
            .await;
        assert_eq!(upload.is_ok(), true);

        let usage = blob_service
            .get_user_resource_usage(&ctx, user_id.clone())
            .await;
        assert_eq!(usage.is_ok(), true);
        assert_eq!(usage.ok().unwrap().blob_usage.usage, 256 as u64);
    }
}
