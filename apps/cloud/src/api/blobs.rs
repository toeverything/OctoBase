use crate::context::Context;
use axum::{
    extract::{BodyStream, Path},
    headers::ContentLength,
    http::{HeaderMap, Method},
    response::{IntoResponse, Response},
    Extension, TypedHeader,
};

use cloud_database::Claims;
use jwst_logger::{info, instrument, tracing};
use std::sync::Arc;

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
    Path((workspace_id, id)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    info!("get_blob_in_workspace enter");
    match ctx
        .blob_service
        .get_blob_in_workspace(&ctx, workspace_id, id, method, headers)
        .await
    {
        Ok((header_map, file)) => match file {
            Some(file) => (header_map, file).into_response(),
            None => header_map.into_response(),
        },
        Err(e) => e.into_response(),
    }
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
    ctx.blob_service
        .upload_blob_in_workspace(&ctx, claims.user.id.clone(), workspace_id, length, stream)
        .await
        .into_response()
}

/// Get workspace's `usage`
/// - Return 200 ok and `usage`.
#[utoipa::path(
    get,
    tag = "resource",
    context_path = "/api/resource",
    path = "/usage",
    responses(
        (status = 200, description = "Return usage", body = Usage,
        example=json!({
            "blob_usage":  {
                "usage": 10,
                "max_usage": 100,
            }
        })),
    )
)]
#[instrument(skip(ctx, claims), fields(user_id = %claims.user.id))]
pub async fn get_user_resource_usage(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    info!("get_user_resource enter");
    ctx.blob_service
        .get_user_resource_usage(&ctx, claims.user.id.clone())
        .await
        .into_response()
}

#[cfg(test)]
mod test {
    use axum::{body::Body, http::StatusCode};
    use axum_test_helper::TestClient;
    use bytes::Bytes;
    use cloud_database::CloudDatabase;
    use futures::{stream, StreamExt};
    use serde_json::json;

    use super::{
        super::{make_rest_route, Context},
        *,
    };

    #[tokio::test]
    async fn test_upload_blob_in_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        // create user
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
        //upload blob in workspace
        let resp_json: serde_json::Value = resp.json().await;
        let workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}/blob", workspace_id);
        let body_stream = Body::wrap_stream(test_data_stream.clone());
        let resp = client
            .put(&url)
            .header("Content-Length", test_data_len.clone().to_string())
            .header("authorization", format!("{}", access_token.clone()))
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
            .put(&url)
            .header("Content-Length", content_length)
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn test_get_blob_in_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        // create user
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
        //upload blob in workspace
        let resp_json: serde_json::Value = resp.json().await;
        let workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}/blob", workspace_id.clone());
        let body_stream = Body::wrap_stream(test_data_stream.clone());
        let resp = client
            .put(&url)
            .header("Content-Length", test_data_len.clone().to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let blob_name = resp.text().await;
        let url = format!(
            "/workspace/{}/blob/{}",
            workspace_id.clone(),
            blob_name.clone()
        );
        let resp = client
            .get(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let mock_url = format!("/workspace/{}/blob/mock_id", workspace_id.clone());
        let resp = client
            .get(&mock_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let mock_url = format!("/workspace/mock_id/blob/{}", blob_name.clone());
        let resp = client
            .get(&mock_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let resp = client
            .get("/workspace/mock_id/blob/mock_id")
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
