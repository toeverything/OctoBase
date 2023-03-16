pub mod blobs;
pub mod permissions;
mod user_channel;
mod ws;

pub use ws::*;

use crate::{context::Context, error_status::ErrorStatus, layer::make_firebase_auth_layer};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
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
pub async fn health_check() -> Response {
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
pub async fn query_user(
    Extension(ctx): Extension<Arc<Context>>,
    Query(payload): Query<UserQuery>,
) -> Response {
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
pub async fn make_token(
    Extension(ctx): Extension<Arc<Context>>,
    Json(payload): Json<MakeToken>,
) -> Response {
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
pub async fn get_workspaces(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
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
pub async fn get_workspace_by_id(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
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
pub async fn update_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<UpdateWorkspace>,
) -> Response {
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
pub async fn delete_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
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
pub async fn get_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
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
pub async fn get_public_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace_id): Path<String>,
) -> Response {
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
        Ok(workspace) => workspace.sync_migration().into_response(),
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
pub async fn search_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<WorkspaceSearchInput>,
) -> Response {
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
