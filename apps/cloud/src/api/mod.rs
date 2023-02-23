mod blobs;
mod permissions;

use axum::{
    extract::{Path, Query},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put, Router},
    Extension, Json,
};
use base64::Engine;
use chrono::{Duration, Utc};
use cloud_database::{
    Claims, MakeToken, RefreshToken, UpdateWorkspace, User, UserQuery, UserToken,
    WorkspaceSearchInput,
};
use http::StatusCode;
use jwst::{error, BlobStorage};
use lib0::any::Any;
use std::sync::Arc;
use tower::ServiceBuilder;

use crate::{
    context::Context, error_status::ErrorStatus, layer::make_firebase_auth_layer,
    utils::URL_SAFE_ENGINE,
};

mod ws;
pub use ws::*;

mod user_channel;
pub use user_channel::*;

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
                .layer(
                    ServiceBuilder::new()
                        .layer(make_firebase_auth_layer(ctx.key.jwt_decode.clone())),
                ),
        )
}

async fn health_check() -> Response {
    StatusCode::OK.into_response()
}

async fn query_user(
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

async fn make_token(
    Extension(ctx): Extension<Arc<Context>>,
    Json(payload): Json<MakeToken>,
) -> Response {
    let (user, refresh) = match payload {
        MakeToken::User(user) => (ctx.db.user_login(user).await, None),
        MakeToken::Google { token } => (
            if let Some(claims) = ctx.decode_google_token(token).await {
                ctx.db.google_user_login(&claims).await.map(Some)
            } else {
                Ok(None)
            },
            None,
        ),
        MakeToken::Refresh { token } => {
            let Ok(input) = URL_SAFE_ENGINE.decode(token.clone()) else {
                return ErrorStatus::BadRequest.into_response();
            };
            let data = match ctx.decrypt_aes(input.clone()) {
                Ok(data) => data,
                Err(_) => return ErrorStatus::BadRequest.into_response(),
            };

            let Some(data) = data else {
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
            let refresh = refresh.unwrap_or_else(|| {
                let refresh = RefreshToken {
                    expires: Utc::now().naive_utc() + Duration::days(180),
                    user_id: user.id.clone(),
                    token_nonce: user.token_nonce.unwrap(),
                };

                let json = serde_json::to_string(&refresh).unwrap();

                let data = ctx.encrypt_aes(json.as_bytes());

                URL_SAFE_ENGINE.encode(data)
            });

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
            let token = ctx.sign_jwt(&claims);

            Json(UserToken { token, refresh }).into_response()
        }
        Ok(None) => ErrorStatus::Unauthorized.into_response(),
        Err(e) => {
            error!("Failed to make token: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

async fn get_workspaces(
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

async fn get_workspace_by_id(
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

async fn update_workspace(
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
                .update_workspace(workspace_id.clone(), ctx.clone());
            Json(data).into_response()
        }
        Ok(None) => ErrorStatus::NotFoundWorkspace(workspace_id).into_response(),
        Err(e) => {
            error!("Failed to update workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

async fn delete_workspace(
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
                .update_workspace(workspace_id.clone(), ctx.clone());
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

async fn get_doc(
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
        Ok(workspace) => workspace.read().await.sync_migration().into_response(),
        Err(e) => {
            error!("Failed to get workspace: {:?}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// Resolves to [`SearchResults`]
///
/// [`SearchResults`]: jwst::SearchResults
async fn search_workspace(
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
