pub mod blobs;
pub mod permissions;
mod user_channel;
mod ws;
pub use ws::*;

use crate::{context::Context, error_status::ErrorStatus, layer::make_firebase_auth_layer};
use axum::{
    extract::{Path, Query},
    http::{StatusCode,header::CONTENT_TYPE, HeaderMap},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put, Router},
    Extension, Json,
};
use chrono::{Duration, Utc};
use cloud_database::{
    Claims, MakeToken, RefreshToken, UpdateWorkspace, User, UserQuery, UserToken,
    WorkspaceSearchInput,
};
use jwst::{error, BlobStorage, JwstError};
use jwst_logger::{instrument, info, tracing};
use lib0::any::Any;
use std::sync::Arc;
pub use user_channel::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        get_workspaces,
        get_workspace_by_id,
        update_workspace,
        delete_workspace,
        search_workspace,
        query_user,
        make_token,
        get_doc,
        get_public_doc,
        health_check,
        blobs::get_blob_in_workspace,
        blobs::upload_blob_in_workspace,
        blobs::get_blob,
        blobs::upload_blob,
        blobs::create_workspace,
        permissions::get_members,
        permissions::invite_member,
        permissions::accept_invitation,
        permissions::leave_workspace,
        permissions::remove_user,
    ),
    tags(
        (name = "Workspace", description = "Read and write remote workspace"),
        (name = "Blob", description = "Read and write blob"),
        (name = "Permission", description = "Read and write permission"),
    )
)]
struct ApiDoc;

pub fn make_api_doc_route(route: Router) -> Router {
    jwst_static::with_api_doc_v3(route, ApiDoc::openapi(), env!("CARGO_PKG_NAME"))
}

pub fn make_rest_route(ctx: Arc<Context>) -> Router {
    Router::new()
        .route("/healthz", get(health_check))
        .route("/user", get(query_user))
        .route("/user/token", post(make_token))
        .route("/blob", put(blobs::upload_blob))
        .route("/blob/:name", get(blobs::get_blob))
        .route("/invitation/:path", post(permissions::accept_invitation))
        .nest_service("/global/sync", get(global_ws_handler))
        .route("/public/doc/:id", get(get_public_doc))
        .route("/public/page/:id/:page_id", get(get_public_page))
        // TODO: Will consider this permission in the future
        .route(
            "/workspace/:id/blob/:name",
            get(blobs::get_blob_in_workspace),
        )
        .nest(
            "/",
            Router::new()
                .route(
                    "/workspace",
                    get(get_workspaces).post(blobs::create_workspace),
                )
                .route(
                    "/workspace/:id",
                    get(get_workspace_by_id)
                        .post(update_workspace)
                        .delete(delete_workspace),
                )
                .route(
                    "/workspace/:id/permission",
                    get(permissions::get_members)
                        .post(permissions::invite_member)
                        .delete(permissions::leave_workspace),
                )
                .route("/workspace/:id/doc", get(get_doc))
                .route("/workspace/:id/search", post(search_workspace))
                .route("/workspace/:id/blob", put(blobs::upload_blob_in_workspace))
                .route("/permission/:id", delete(permissions::remove_user))
                .layer(make_firebase_auth_layer(ctx.key.jwt_decode.clone())),
        )
}

///  Health check.
/// - Return 200 Ok.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api",
    path = "/healthz",
    responses(
        (status = 200, description = "Healthy")
    )
)]
#[instrument]
pub async fn health_check() -> Response {
    info!("Health check enter");
    StatusCode::OK.into_response()
}

///  Get `user`'s data by email.
/// - Return 200 ok and `user`'s data.
/// - Return 400 bad request if `email` or `workspace_id` is not provided.
/// - Return 500 internal server error if database error.
#[utoipa::path(get, tag = "Workspace", context_path = "/api", path = "/user",
params(
    ("email",Query, description = "email of user" ),
    ( "workspace_id",Query, description = "workspace id of user")),
    responses(
        (status = 200, description = "Return workspace data", body = [UserInWorkspace],example = json!([
            {
              "type": "UnRegistered",
              "email": "toeverything@toeverything.info",
              "in_workspace": false
            }
          ])),
        (status = 400, description = "Request parameter error."),
        (status = 500, description = "Server error, please try again later.")
    )
    
)]
#[instrument(skip(ctx))]
pub async fn query_user(
    Extension(ctx): Extension<Arc<Context>>,
    Query(payload): Query<UserQuery>,
) -> Response {
    info!("query_user enter");
    if let (Some(email), Some(workspace_id)) = (payload.email, payload.workspace_id) {
        if let Ok(user) = ctx
            .db
            .get_user_in_workspace_by_email(workspace_id, &email)
            .await
        {
            Json(vec![user]).into_response()
        } else {
            ErrorStatus::InternalServerError.into_response()
        }
    } else {
        ErrorStatus::BadRequest.into_response()
    }
}

///  create `token` for user.
/// - Return 200 ok and `token`.
/// - Return 400 bad request if request parameter error.
/// - Return 401 unauthorized if user unauthorized.
/// - Return 500 internal server error if database error.
#[utoipa::path(post, tag = "Workspace", context_path = "/api/user", path = "/token",
request_body(content = MakeToken, description = "Request body for make token",content_type = "application/json",example = json!({
    "type": "Google",
    "token": "google token",
}
)),
responses(
    (status = 200, description = "Return token", body = UserToken,
    example=json!({
        "refresh":"refresh token",
        "token":"token",
    }
    )),
    (status = 400, description = "Request parameter error."),
    (status = 401, description = "Unauthorized."),
    (status = 500, description = "Server error, please try again later.")
)
)]
#[instrument(skip(ctx, payload))]  // payload need to be safe
pub async fn make_token(
    Extension(ctx): Extension<Arc<Context>>,
    Json(payload): Json<MakeToken>,
) -> Response {
    info!("make_token enter");
    // TODO: too complex type, need to refactor
    let (user, refresh) = match payload {
        MakeToken::DebugCreateUser(user) => {
            if cfg!(debug_assertions) || std::env::var("JWST_DEV").is_ok() {
                if let Ok(model) = ctx.db.create_user(user).await {
                    (Ok(Some(model)), None)
                } else {
                    return ErrorStatus::BadRequest.into_response();
                }
            } else {
                return ErrorStatus::BadRequest.into_response();
            }
        }
        MakeToken::DebugLoginUser(user) => {
            if cfg!(debug_assertions) || std::env::var("JWST_DEV").is_ok() {
                (ctx.db.user_login(user).await, None)
            } else {
                return ErrorStatus::BadRequest.into_response();
            }
        }
        MakeToken::Google { token } => (
            if let Some(claims) = ctx.firebase.lock().await.decode_google_token(token).await {
                ctx.db.google_user_login(&claims).await.map(Some)
            } else {
                Ok(None)
            },
            None,
        ),
        MakeToken::Refresh { token } => {
            let Ok(data) = ctx.key.decrypt_aes_base64(token.clone()) else {
                return ErrorStatus::BadRequest.into_response();
            };

            let Ok(data) = serde_json::from_slice::<RefreshToken>(&data) else {
                return ErrorStatus::BadRequest.into_response();
            };

            if data.expires < Utc::now().naive_utc() {
                return ErrorStatus::Unauthorized.into_response();
            }

            (ctx.db.refresh_token(data).await, Some(token))
        }
    };

    match user {
        Ok(Some(user)) => {
            let Some(refresh) = refresh.or_else(|| {
                let refresh = RefreshToken {
                    expires: Utc::now().naive_utc() + Duration::days(180),
                    user_id: user.id.clone(),
                    token_nonce: user.token_nonce.unwrap(),
                };

                let json = serde_json::to_string(&refresh).unwrap();

                ctx.key.encrypt_aes_base64(json.as_bytes()).ok()
            }) else {
                return ErrorStatus::InternalServerError.into_response();
            };

            let claims = Claims {
                exp: Utc::now().naive_utc() + Duration::minutes(10),
                user: User {
                    id: user.id,
                    name: user.name,
                    email: user.email,
                    avatar_url: user.avatar_url,
                    created_at: user.created_at.unwrap_or_default().naive_local(),
                },
            };
            let token = ctx.key.sign_jwt(&claims);

            Json(UserToken { token, refresh }).into_response()
        }
        Ok(None) => ErrorStatus::Unauthorized.into_response(),
        Err(e) => {
            error!("Failed to make token: {:?}", e);
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
        .get_permission(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(Some(_)) => (),
        Ok(None) => return ErrorStatus::Forbidden.into_response(),
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


/// Get a exists `page` json by page id
/// - Return `page` json.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/workspace",
    path = "/{workspace_id}/page/{page_id}",
    params(
        ("workspace_id", description = "workspace id"),
        ("page_id", description = "page id")
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
        Ok(true) => (),
        Ok(false) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {:?}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    }

    match ctx.storage.get_workspace(workspace_id).await {
        Ok(workspace) => {
            if headers
                .get(CONTENT_TYPE)
                .and_then(|c| c.to_str().ok())
                .map(|s| s.contains("json"))
                .unwrap_or(false)
            {
                if let Some(space) = workspace.with_trx(|t| t.get_exists_space(page_id)) {
                    Json(space).into_response()
                } else {
                    ErrorStatus::NotFound.into_response()
                }
            } else if let Some(markdown) = workspace.with_trx(|t| {
                t.get_exists_space(page_id)
                    .and_then(|page| page.to_markdown(&t.trx))
            }) {
                markdown.into_response()
            } else {
                ErrorStatus::NotFound.into_response()
            }
        }
        Err(JwstError::WorkspaceNotFound(_)) => ErrorStatus::NotFound.into_response(),
        Err(e) => {
            error!("Failed to get workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}


/// Get a exists `public doc` by workspace id
/// - Return 200 ok and `public doc` .
/// - Return 403 Forbidden if you do not have permission.
/// - Return 404 Not Found if `Workspace` is not exists.
/// - Return 500 Internal Server Error if database error.
#[utoipa::path(
    get,
    tag = "Workspace",
    context_path = "/api/public",
    path = "/doc/{workspace_id}",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Successfully get public doc.", body =Vec<u8>,),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 404, description = "Workspace not found."),
        (status = 500, description = "Server error, please try again later.")
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
            if let Some(update) = workspace.sync_migration(50) {
                update.into_response()
            } else {
                ErrorStatus::NotFound.into_response()
            }
        },
        Err(JwstError::WorkspaceNotFound(_)) => ErrorStatus::NotFound.into_response(),
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
    use super::*;
    use axum:: body::Body;
    use axum_test_helper::TestClient;
    use bytes::Bytes;
    use cloud_database::{CloudDatabase, CreateUser};
    use futures::{stream, StreamExt};
    use serde_json::json;
    use std::sync::Arc;
    #[tokio::test]
    async fn test_health_check() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let resp = client.get("/healthz").send().await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_query_user() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test(pool).await;
        let new_user = context
            .db
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let url = format!(
            "/user?email={}&workspace_id=212312",
            new_user.email
        );
        let resp = client.get(&url).send().await;
      
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        assert!(resp_text.contains(new_user.id.as_str()));
        assert!(resp_text.contains(new_user.name.as_str()));
        assert!(resp_text.contains(new_user.email.as_str()));

        let resp = client.get("/user").send().await;
        assert_eq!(resp.status().is_client_error(), true);
        let resp = client.get("/user?email=fake_email").send().await;
        assert_eq!(resp.status().is_client_error(), true);
        let resp = client.get("/user?user_name=fake_parameter").send().await;
        assert_eq!(resp.status().is_client_error(), true);
    }

    #[tokio::test]
    async fn test_make_token_with_valid_request() {
    let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
    let context = Context::new_test(pool).await;
    let ctx = Arc::new(context);
    let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

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
    let refresh_token = resp_json["refresh"].as_str().unwrap().to_string();
        let body_data = json!({
            "type": "Refresh",
            "token": refresh_token,
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_make_token_with_invalid_request() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));
    
        let client = TestClient::new(app);
        let body_data = json!({
            "type": "DebugCreateUser",
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
             assert_eq!(resp.status().is_client_error(), true);
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "my_email",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
             assert_eq!(resp.status().is_client_error(), true);
        let body_data = json!({
            "type": "Refresh",
            "token": "my_token",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status().is_client_error(), true);
        let body_data = json!({
            "type": "Google",
            "token": "my_token",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status().is_client_error(), true);
    }
    #[tokio::test]
    async fn test_upload_blob() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(test_data.into_iter().map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))));
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
            .take(num_chunks).map(Ok::<_, std::io::Error>);
    
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
        let context = Context::new_test(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(test_data.into_iter().map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))));
        let body_stream = Body::wrap_stream(test_data_stream);

        let resp = client
            .put("/blob")
            .header("Content-Length", test_data_len.to_string())
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let blob_name=resp.text().await;
        let url = format!(
            "/blob/{}",
            blob_name
        );
        let resp = client
            .get(&url)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .get( "/blob/mock_id",)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

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
        let test_data_stream = stream::iter(test_data.into_iter().map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))));
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
 
}
