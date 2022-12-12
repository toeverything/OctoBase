use std::sync::Arc;

use axum::{
    extract::{Path, Query},
    response::{IntoResponse, Response},
    routing::{delete, get, post, Router},
    Extension, Json,
};
use chrono::{Duration, Utc};
use http::StatusCode;
use lettre::{message::Mailbox, AsyncTransport, Message};
use tower::ServiceBuilder;

use crate::{
    context::Context,
    layer::make_firebase_auth_layer,
    model::{
        Claims, CreatePermission, CreateWorkspace, MakeToken, PermissionType, RefreshToken,
        UpdateWorkspace, UserCred, UserQuery, UserToken, UserWithNonce,
    },
};

mod ws;

pub fn make_rest_route(ctx: Arc<Context>) -> Router {
    Router::new()
        .route("/user/token", post(make_token))
        .route("/invitation/:path", post(accept_invitation))
        .nest(
            "/",
            Router::new()
                .route("/user", get(query_user))
                .route("/workspace", get(get_workspaces).post(create_workspace))
                .route(
                    "/workspace/:id",
                    get(get_workspace_by_id)
                        .post(update_workspace)
                        .delete(delete_workspace),
                )
                .route(
                    "/workspace/:id/permission",
                    get(get_members).post(create_permission),
                )
                .route("/permission/:id", delete(delete_permission))
                .layer(
                    ServiceBuilder::new()
                        .layer(make_firebase_auth_layer(ctx.key.jwt_decode.clone())),
                ),
        )
}

async fn query_user(
    Extension(ctx): Extension<Arc<Context>>,
    Query(payload): Query<UserQuery>,
) -> Response {
    if payload.email.is_none() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match ctx.query_user(payload).await {
        Ok(Some(user)) => Json(user).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn make_token(
    Extension(ctx): Extension<Arc<Context>>,
    Json(payload): Json<MakeToken>,
) -> Response {
    let (user, refresh) = match payload {
        MakeToken::User(user) => (ctx.user_login(user).await, None),
        MakeToken::Google { token } => (
            if let Some(claims) = ctx.decode_google_token(token).await {
                ctx.google_user_login(&claims).await.map(|user| Some(user))
            } else {
                Ok(None)
            },
            None,
        ),
        MakeToken::Refresh { token } => {
            let Ok(input) = base64::decode_config(token.clone(), base64::STANDARD_NO_PAD) else {
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

            (ctx.refresh_token(data).await, Some(token))
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

                base64::encode_config(data, base64::STANDARD_NO_PAD)
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
    if let Ok(data) = ctx.get_user_workspaces(claims.user.id).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn get_workspace_by_id(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i32>,
) -> Response {
    match ctx.get_permission(claims.user.id, id).await {
        Ok(Some(_)) => (),
        Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.get_workspace_by_id(id).await {
        Ok(data) => Json(data).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn create_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Json(payload): Json<CreateWorkspace>,
) -> Response {
    if let Ok(data) = ctx.create_normal_workspace(claims.user.id, payload).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn update_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateWorkspace>,
) -> Response {
    match ctx.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.update_workspace(id, payload).await {
        Ok(Some(data)) => Json(data).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i32>,
) -> Response {
    match ctx.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.is_owner() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.delete_workspace(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_members(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i32>,
) -> Response {
    match ctx.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Ok(members) = ctx.get_workspace_members(id).await {
        Json(members).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn create_permission(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i32>,
    Json(data): Json<CreatePermission>,
) -> Response {
    match ctx.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let Ok(addr) = data.email.clone().parse() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    let (permission_id, user_cred) = match ctx
        .create_permission(&data.email, id, PermissionType::Write)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::CONFLICT.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let encrypted = ctx.encrypt_aes(&permission_id.to_le_bytes()[..]);

    let path = base64::encode_config(encrypted, base64::URL_SAFE_NO_PAD);

    let mailbox = Mailbox::new(
        match user_cred {
            UserCred::Registered(user) => Some(user.name),
            UserCred::UnRegistered { .. } => None,
        },
        addr,
    );

    let email = Message::builder()
        .from(ctx.mail.mail_box.clone())
        .to(mailbox)
        .subject(&ctx.mail.title)
        .body(path)
        .unwrap();

    match ctx.mail.client.send(email.clone()).await {
        Ok(_) => {}
        // TODO: https://github.com/lettre/lettre/issues/743
        Err(e) if e.is_response() => {
            if let Err(_) = ctx.mail.client.send(email).await {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    StatusCode::OK.into_response()
}

async fn accept_invitation(
    Extension(ctx): Extension<Arc<Context>>,
    Path(url): Path<String>,
) -> Response {
    let Ok(input) = base64::decode_config(url, base64::URL_SAFE_NO_PAD) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Some(data) = ctx.decrypt_aes(input) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(data) = TryInto::<[u8; 4]>::try_into(data) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match ctx.accept_permission(i32::from_le_bytes(data)).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_permission(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i32>,
) -> Response {
    match ctx.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.delete_permission(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
