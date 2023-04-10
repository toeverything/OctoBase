use crate::{context::Context, infrastructure::error_status::ErrorStatus};
use axum::{
    extract::{BodyStream, Path},
    headers::ContentLength,
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json, TypedHeader,
};
use cloud_database::{Claims, UpdateWorkspace, WorkspaceSearchInput};
use futures::{future, StreamExt};
use jwst::{error, BlobStorage};
use jwst_logger::{info, instrument, tracing};
use jwst_storage::JwstStorageError;
use std::sync::Arc;

impl Context {
    #[instrument(skip(self, stream))]
    pub(super) async fn upload_workspace(&self, stream: BodyStream) -> Vec<u8> {
        info!("upload_workspace enter");
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

/// Create `Workspace` .
/// - Return 200 ok and `Workspace`'s data.
/// - Return 500 internal server error.
#[utoipa::path(post, tag = "Workspace", context_path = "/api", path = "/workspace",
request_body(content = BodyStream, description = "Request body for updateWorkspace",content_type="application/octet-stream"),
    responses(
        (status = 200, description = "Successfully create workspace",body=Workspace,example=json!({
            "id": "xxx",
            "public": false,
            "type": 1,
            "created_at": "1677122059817"
        }
        )),
        (status = 500, description = "Internal server error"),
    ),
)]
#[instrument(skip(ctx, claims, _length, stream), fields(user_id = %claims.user.id))]
pub async fn create_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    TypedHeader(_length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    info!("create_workspace enter");
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

/// Get user's `Workspace` .
/// - Return 200 ok and `Workspace`'s data.
/// - Return 500 internal server error if database error.
#[utoipa::path(get, tag = "Workspace", context_path = "/api", path = "/workspace", responses(
    (status = 200, description = "Workspace's data", body = Vec<WorkspaceWithPermission>,
    example=json!([{
        "permission": 1,
        "id": "xxxx",
        "public": true,
        "type": 1
    }]
    )),
    (status = 500, description = "Server error, please try again later.")
))]
#[instrument(
    skip(ctx, claims),
    fields(
        user_id = %claims.user.id,
    )
)]
pub async fn get_workspaces(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    info!("get_workspaces enter");
    // TODO should print error
    match ctx.db.get_user_workspaces(claims.user.id.clone()).await {
        Ok(data) => Json(data).into_response(),
        Err(e) => {
            error!("Failed to get workspaces: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}
/// Get a exists `Workspace` by id
/// - Return 200 Ok and `Workspace`'s data if `Workspace` is exists.
/// - Return 403 Forbidden if you do not have permission.
/// - Return 500 Internal Server Error if database error.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/workspace",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Get workspace data", body = WorkspaceDetail),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 500, description = "Server error, please try again later.")
    )
)]
#[instrument(
    skip(ctx, claims),
    fields(
        user_id = %claims.user.id,
    )
)]
pub async fn get_workspace_by_id(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    info!("get_workspace_by_id enter");
    match ctx
        .db
        .can_read_workspace(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(true) => (),
        Ok(false) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    match ctx.db.get_workspace_by_id(workspace_id.clone()).await {
        Ok(Some(data)) => Json(data).into_response(),
        Ok(None) => ErrorStatus::NotFoundWorkspace(workspace_id).into_response(),
        Err(e) => {
            error!("Failed to get workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// update a exists `Workspace` by id
/// - Return 200 ok and `Workspace`'s data.
/// - Return 403 Forbidden if you do not have permission.
/// - Return 404 Not Found if `Workspace` is not exists.
/// - Return 500 Internal Server Error if database error.
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/workspace",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    request_body(content = UpdateWorkspace, description = "Request body for updateWorkspace",content_type = "application/json",example = json!({
        "public":true})),
    responses(
        (status = 200, description = "Return Workspace", body = Workspace,
        example=json!({
            "id": "xxx",
            "public": true,
            "type": 1,
            "created_at": "1677122059817"
        }
        )),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 404, description = "Workspace not found."),
        (status = 500, description = "Server error, please try again later.")
    )
)]
#[instrument(
    name = "update_workspace",
    skip(ctx, claims),
    fields(
        user_id = %claims.user.id,
    )
)]
pub async fn update_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<UpdateWorkspace>,
) -> Response {
    info!("update_workspace enter");
    match ctx
        .db
        .get_permission(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    match ctx.db.update_workspace(workspace_id.clone(), payload).await {
        Ok(Some(data)) => {
            ctx.user_channel
                .update_workspace(workspace_id.clone(), ctx.clone())
                .await;
            Json(data).into_response()
        }
        Ok(None) => ErrorStatus::NotFoundWorkspace(workspace_id).into_response(),
        Err(e) => {
            error!("Failed to update workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// Delete a exists `Workspace` by id
/// - Return 200 ok.
/// - Return 403 Forbidden if you do not have permission.
/// - Return 404 Not Found if `Workspace` is not exists.
/// - Return 500 Internal Server Error if database error.
#[utoipa::path(
    delete,
    tag = "Workspace",
    context_path = "/api/workspace",
    path = "/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Successfully deleted workspace."),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 404, description = "Workspace not found."),
        (status = 500, description = "Server error, please try again later.")
    )
)]
#[instrument(
    name = "delete_workspace",
    skip(ctx, claims),
    fields(
        user_id = %claims.user.id,
    )
)]
pub async fn delete_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    info!("delete_workspace enter");
    match ctx
        .db
        .get_permission(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.is_owner() => (),
        Ok(_) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    match ctx.db.delete_workspace(workspace_id.clone()).await {
        Ok(true) => {
            ctx.user_channel
                .update_workspace(workspace_id.clone(), ctx.clone())
                .await;
            ctx.close_websocket_by_workspace(workspace_id.clone()).await;

            let _ = ctx.storage.blobs().delete_workspace(workspace_id).await;
            StatusCode::OK.into_response()
        }
        Ok(false) => ErrorStatus::NotFoundWorkspace(workspace_id).into_response(),
        Err(e) => {
            error!("Failed to delete workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// Get a exists `doc` by workspace id
/// - Return 200 ok and `doc` .
/// - Return 403 Forbidden if you do not have permission.
/// - Return 404 Not Found if `Workspace` is not exists.
/// - Return 500 Internal Server Error if database error.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/workspace",
    path = "/{workspace_id}/doc",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Successfully get doc.", body =Vec<u8>,),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 404, description = "Workspace not found."),
        (status = 500, description = "Server error, please try again later.")
    )
)]
#[instrument(
    skip(ctx, claims),
    fields(
        user_id = %claims.user.id
    )
)]
pub async fn get_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    info!("get_doc enter");
    match ctx
        .db
        .can_read_workspace(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(true) => (),
        Ok(false) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    get_workspace_doc(ctx, workspace_id).await
}

/// Get a exists `page` markdown/json/doc by page id
/// - If Context-Type is `application/json` then return `page` json.
/// - If Context-Type starts with `text/` then return `page` markdown.
/// - If neither then return `page` single page doc.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/public",
    path = "/workspace/{workspace_id}/{page_id}",
    params(
        ("workspace_id", description = "workspace id"),
        ("page_id", description = "page id")
    ),
    responses(
        (status = 200, description = "Successfully get public page.", body =Vec<u8>,),
        (status = 403, description = "Not a public workspace or page."),
        (status = 404, description = "Workspace not found."),
        (status = 500, description = "Server internal error.")
    )
)]
#[instrument(skip(ctx, headers))]
pub async fn get_public_page(
    Extension(ctx): Extension<Arc<Context>>,
    headers: HeaderMap,
    Path((workspace_id, page_id)): Path<(String, String)>,
) -> Response {
    info!("get_page enter");
    match ctx.db.is_public_workspace(workspace_id.clone()).await {
        // check if page is public
        // TODO: improve logic
        Ok(false)
            if ctx
                .storage
                .get_workspace(workspace_id.clone())
                .await
                .ok()
                .and_then(|ws| {
                    ws.try_with_trx(|mut t| t.get_space(page_id.clone()).shared(&t.trx))
                })
                != Some(true) =>
        {
            return ErrorStatus::Forbidden.into_response();
        }
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
        Ok(true | false) => (),
    }

    match ctx.storage.get_workspace(workspace_id).await {
        Ok(workspace) => {
            if let Some(space) = workspace.with_trx(|t| t.get_exists_space(page_id)) {
                // TODO: check page level permission
                match headers.get(CONTENT_TYPE).and_then(|c| c.to_str().ok()) {
                    Some("application/json") => Json(space).into_response(),
                    Some(mine) if mine.starts_with("text/") => {
                        if let Some(markdown) = workspace
                            .retry_with_trx(|t| space.to_markdown(&t.trx), 10)
                            .ok()
                            .flatten()
                        {
                            markdown.into_response()
                        } else {
                            ErrorStatus::InternalServerError.into_response()
                        }
                    }
                    _ => {
                        if let Some(doc) = workspace
                            .retry_with_trx(|t| space.to_single_page(&t.trx).ok(), 10)
                            .ok()
                            .flatten()
                        {
                            doc.into_response()
                        } else {
                            ErrorStatus::InternalServerError.into_response()
                        }
                    }
                }
            } else {
                ErrorStatus::NotFound.into_response()
            }
        }
        Err(JwstStorageError::WorkspaceNotFound(_)) => ErrorStatus::NotFound.into_response(),
        Err(e) => {
            error!("Failed to get workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// Get a exists `public doc` by workspace id
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/public",
    path = "/workspace/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Successfully get public doc.", body =Vec<u8>,),
        (status = 403, description = "Not a public workspace."),
        (status = 404, description = "Workspace not found."),
        (status = 500, description = "Server internal error.")
    )
)]
#[instrument(skip(ctx))]
pub async fn get_public_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace_id): Path<String>,
) -> Response {
    info!("get_public_doc enter");
    match ctx.db.is_public_workspace(workspace_id.clone()).await {
        Ok(true) => (),
        Ok(false) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    get_workspace_doc(ctx, workspace_id).await
}

async fn get_workspace_doc(ctx: Arc<Context>, workspace_id: String) -> Response {
    match ctx.storage.get_workspace(workspace_id).await {
        Ok(workspace) => {
            if let Ok(update) = workspace.sync_migration(50) {
                update.into_response()
            } else {
                ErrorStatus::NotFound.into_response()
            }
        }
        Err(JwstStorageError::WorkspaceNotFound(_)) => ErrorStatus::NotFound.into_response(),
        Err(e) => {
            error!("Failed to get workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// Resolves to [`SearchResults`]
///
/// [`SearchResults`]: jwst::SearchResults
/// search in workspace
/// - Return block id
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/workspace",
    path = "/{workspace_id}/search",
    request_body(content = WorkspaceSearchInput, description = "Request body for search workspace",content_type = "application/json",example = json!({
        "query": "string",
    }
    )),
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Workspace's data", body = SearchResults,
        example=json!([{
         "block_id": "xxxx",
         "score": "f32",
        }]
        )),
        (status = 400, description = "Request parameter error."),
        (status = 401, description = "Unauthorized."),
        (status = 500, description = "Server error, please try again later.")
    )
)]
#[instrument(
    skip(ctx, claims),
    fields(
        user_id = %claims.user.id,
    )
)]
pub async fn search_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<WorkspaceSearchInput>,
) -> Response {
    info!("search_workspace enter");
    match ctx
        .db
        .can_read_workspace(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(true) => (),
        Ok(false) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    };

    let search_results = match ctx.search_workspace(workspace_id, &payload.query).await {
        Ok(results) => results,
        Err(err) => return err.to_string().into_response(),
    };

    Json(search_results).into_response()
}

#[cfg(test)]
mod test {
    use super::super::{make_rest_route, Context};
    use axum::{body::Body, http::StatusCode, Extension};
    use axum_test_helper::TestClient;
    use bytes::Bytes;
    use cloud_database::CloudDatabase;
    use futures::stream;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_create_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
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
        let body_clone = Body::wrap_stream(test_data_stream.clone());

        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .body(body_clone)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_get_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
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

        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let create_workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let resp = client
            .get("/workspace")
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        assert!(resp_text.contains(&create_workspace_id));
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text).unwrap();
        let first_object = resp_json[0].as_object().unwrap();
        let permission = first_object["permission"].as_i64().unwrap();
        assert_eq!(permission, 99);
        let resp = client.get("/workspace").send().await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_get_workspace_by_id() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
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

        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let create_workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}", create_workspace_id);
        let resp = client
            .get(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.text().await.contains(&create_workspace_id));
        let resp = client
            .get("/workspace/invalid_id")
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_update_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
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

        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let create_workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}", create_workspace_id);
        let body_data = json!({
            "public": true,
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let post_workspace_id = resp_json["id"].as_str().unwrap().to_string();
        assert_eq!(create_workspace_id, post_workspace_id);
        let workspace_public = resp_json["public"].as_bool().unwrap();
        assert_eq!(workspace_public, true);
        let resp = client
            .post("/workspace/mock_id")
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let resp = client
            .post("/workspace/mock_id")
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
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

        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let create_workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}", create_workspace_id);
        let resp = client
            .delete(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .get(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_get_doc() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
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

        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}/doc", workspace_id);
        let resp = client
            .get(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .get("/workspace/mock_id/doc")
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_get_public_doc() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        //create user
        let client = TestClient::new(app);
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

        //login user
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

        //create workspace
        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let public_url = format!("/public/workspace/{}", workspace_id.clone());
        let resp = client
            .get(&public_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        //public workspace
        let url = format!("/workspace/{}", workspace_id.clone());
        let body_data = json!({
            "public": true,
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let is_workspace_public = resp_json["public"].as_bool().unwrap();
        assert_eq!(is_workspace_public, true);

        // check public workspace
        let url = format!("/public/workspace/{}", workspace_id);
        let resp = client
            .get(&url)
            .header("Content-Type", "application/json")
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        // TODO: check public page

        //get not public doc
        let resp = client
            .get(&public_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .get("/public/workspace/mock_id")
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
