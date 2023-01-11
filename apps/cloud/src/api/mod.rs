mod blobs;
mod permissions;

use axum::{
    extract::{Path, Query},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put, Router},
    Extension, Json,
};
use chrono::{Duration, Utc};
use http::StatusCode;
use jwst::{BlobStorage, Workspace as JWSTWorkspace};
use jwst_storage::{
    Claims, CreateWorkspace, MakeToken, RefreshToken, UpdateWorkspace, UserQuery, UserToken,
    UserWithNonce, WorkspaceSearchInput,
};
use lib0::any::Any;
use std::sync::Arc;
use tower::ServiceBuilder;
use yrs::StateVector;

use crate::{
    context::{Context, ContextRequestError},
    layer::make_firebase_auth_layer,
    login::ThirdPartyLogin,
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
        .route("/global/sync", get(global_ws_handler))
        .route("/public/doc/:id", get(get_public_doc))
        .nest(
            "/",
            Router::new()
                .route("/workspace", get(get_workspaces).post(create_workspace))
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
                .route(
                    "/workspace/:id/blob/:name",
                    get(blobs::get_blob_in_workspace),
                )
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
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    } else {
        StatusCode::BAD_REQUEST.into_response()
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
                ctx.google_user_login(&claims).await.map(|user| Some(user))
            } else {
                Ok(None)
            },
            None,
        ),
        MakeToken::Refresh { token } => {
            let Ok(input) = base64::decode_engine(token.clone(), &URL_SAFE_ENGINE) else {
                return StatusCode::BAD_REQUEST.into_response();
            };
            let Some(data) = ctx.decrypt_aes(input) else {
                return StatusCode::BAD_REQUEST.into_response();
            };
            let Ok(data) = serde_json::from_slice::<RefreshToken>(&data) else {
                return StatusCode::BAD_REQUEST.into_response();
            };

            if data.expires < Utc::now().naive_utc() {
                return StatusCode::GONE.into_response();
            }

            (ctx.db.refresh_token(data).await, Some(token))
        }
    };

    match user {
        Ok(Some(UserWithNonce { user, token_nonce })) => {
            let refresh = refresh.unwrap_or_else(|| {
                let refresh = RefreshToken {
                    expires: Utc::now().naive_utc() + Duration::days(180),
                    user_id: user.id,
                    token_nonce,
                };

                let json = serde_json::to_string(&refresh).unwrap();

                let data = ctx.encrypt_aes(json.as_bytes());

                base64::encode_engine(data, &URL_SAFE_ENGINE)
            });

            let claims = Claims {
                exp: Utc::now().naive_utc() + Duration::minutes(10),
                user,
            };
            let token = ctx.sign_jwt(&claims);

            Json(UserToken { token, refresh }).into_response()
        }
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_workspaces(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    // TODO should print error
    if let Ok(data) = ctx.db.get_user_workspaces(claims.user.id).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

impl IntoResponse for ContextRequestError {
    fn into_response(self) -> Response {
        match self {
            ContextRequestError::BadUserInput { user_message } => {
                (StatusCode::BAD_REQUEST, user_message).into_response()
            }
            ContextRequestError::WorkspaceNotFound { workspace_id } => (
                StatusCode::NOT_FOUND,
                format!("Workspace({workspace_id:?}) not found."),
            )
                .into_response(),
            ContextRequestError::Other(err) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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
        .get_permission(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(Some(_)) => (),
        Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.get_workspace_by_id(workspace_id).await {
        Ok(Some(data)) => Json(data).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn create_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Json(payload): Json<CreateWorkspace>,
) -> Response {
    if let Ok(data) = ctx.db.create_normal_workspace(claims.user.id).await {
        let doc = {
            let doc = JWSTWorkspace::new(data.id.to_string());

            doc.with_trx(|mut t| {
                t.set_metadata("name", Any::String(payload.name.into_boxed_str()));
            });
            doc
        };
        let id = data.id.to_string();
        if let Err(_) = ctx.docs.create_doc(&id).await {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let update = doc.sync_migration();
        if let Err(_) = ctx.docs.full_migrate(&id, update).await {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        ctx.user_channel
            .add_user_observe(claims.user.id, ctx.clone())
            .await;
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
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
        .get_permission(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.update_workspace(workspace_id.clone(), payload).await {
        Ok(Some(data)) => {
            ctx.user_channel
                .update_workspace(workspace_id.clone(), ctx.clone());
            Json(data).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    match ctx
        .db
        .get_permission(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.is_owner() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.delete_workspace(workspace_id.clone()).await {
        Ok(true) => {
            ctx.user_channel
                .update_workspace(workspace_id.clone(), ctx.clone());
            let _ = ctx.blob.delete_workspace(workspace_id).await;
            StatusCode::OK.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    match ctx
        .db
        .can_read_workspace(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    get_workspace_doc(ctx, workspace_id).await
}

pub async fn get_public_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace_id): Path<String>,
) -> Response {
    match ctx.db.is_public_workspace(workspace_id.clone()).await {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    get_workspace_doc(ctx, workspace_id).await
}

async fn get_workspace_doc(ctx: Arc<Context>, workspace_id: String) -> Response {
    if let Some(doc) = ctx.doc.try_get_workspace(workspace_id.clone()) {
        return doc
            .read()
            .await
            .doc()
            .encode_state_as_update_v1(&StateVector::default())
            .into_response();
    }

    let id = workspace_id.to_string();
    match ctx.docs.create_doc(&id).await {
        Ok(doc) => doc
            .encode_state_as_update_v1(&StateVector::default())
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Resolves to [WorkspaceSearchResults]
async fn search_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    Json(payload): Json<WorkspaceSearchInput>,
) -> Response {
    match ctx
        .db
        .can_read_workspace(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let search_results = match ctx.search_workspace(workspace_id, &payload.query).await {
        Ok(results) => results,
        Err(err) => return err.into_response(),
    };

    Json(search_results).into_response()
}
