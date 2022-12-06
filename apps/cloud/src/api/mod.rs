use std::sync::Arc;

use aes_gcm::{aead::Aead, Nonce};
use axum::{
    extract::Path,
    response::{IntoResponse, Response},
    routing::{get, post, Router},
    Extension, Json,
};
use chrono::{Duration, Utc};
use http::StatusCode;
use lettre::{message::Mailbox, AsyncTransport, Message};
use rand::{thread_rng, Rng};
use tower::ServiceBuilder;

use crate::{
    context::Context,
    database::CreatePrivateWorkspaceError,
    layer::make_firebase_auth_layer,
    model::{
        Claims, CreatePermission, CreateWorkspace, Invitation, PermissionType, UpdateWorkspace,
        UserCredType,
    },
};

mod ws;

pub fn make_rest_route(ctx: Arc<Context>) -> Router {
    Router::new()
        .route("/workspace", get(get_workspaces).post(create_workspace))
        .route(
            "/workspace/:id",
            get(get_workspace_by_id).post(update_workspace),
        )
        .route("/user", post(init_user))
        .route("/invitation/:path", post(accept_invitation))
        .route(
            "/workspace/:id/permission",
            get(get_members)
                .post(create_permission)
                .delete(delete_permission),
        )
        .layer(ServiceBuilder::new().layer(make_firebase_auth_layer(ctx.http_client.clone())))
}

async fn get_workspaces(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    if let Ok(data) = ctx.get_user_workspaces(&claims.user_id).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn get_workspace_by_id(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i32>() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    match ctx.get_permission(&claims.user_id, id).await {
        Ok(Some(_)) => (),
        Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    match ctx.get_workspace_by_id(id).await {
        Ok(data) => Json(data).into_response(),
        Err(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn create_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Json(payload): Json<CreateWorkspace>,
) -> Response {
    if let Ok(data) = ctx.create_normal_workspace(&claims.user_id, payload).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn init_user(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    match ctx.init_user(&claims.user_id).await {
        Ok(Ok(data)) => Json(data).into_response(),
        Ok(Err(CreatePrivateWorkspaceError::Exist)) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn update_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWorkspace>,
) -> Response {
    let Ok(id) = id.parse::<i32>() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    match ctx.get_permission(&claims.user_id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    match ctx.update_workspace(id, payload).await {
        Ok(data) => Json(data).into_response(),
        Err(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_members(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i32>() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    match ctx.get_permission(&claims.user_id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match ctx.get_workspace_members(id).await {
        Ok(members) => Json(members).into_response(),
        Err(e) => {
            println!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn create_permission(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
    Json(data): Json<CreatePermission>,
) -> Response {
    let Ok(id) = id.parse::<i32>() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    match ctx.get_permission(&claims.user_id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let permission_id = match ctx
        .create_permission(&data, id, PermissionType::Write)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::CONFLICT.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let inv = Invitation {
        id: permission_id,
        expire: Utc::now().naive_utc() + Duration::days(1),
    };

    let inv = serde_json::to_string(&inv).unwrap();
    let rand_data: [u8; 12] = thread_rng().gen();
    let nonce = Nonce::from_slice(&rand_data);

    let mut encrypted = ctx.aes_key.encrypt(nonce, inv.as_bytes()).unwrap();
    encrypted.extend(nonce);

    let path = base64::encode_config(encrypted, base64::URL_SAFE_NO_PAD);

    let mailbox = match data.user_cred_type {
        UserCredType::Id => todo!(),
        UserCredType::Email => {
            let Ok(addr) = data.user_cred.parse() else {
                return StatusCode::BAD_REQUEST.into_response()
            };
            Mailbox::new(None, addr)
        }
    };

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
    Extension(claims): Extension<Arc<Claims>>,
    Path(url): Path<String>,
) -> Response {
    let Ok(input) = base64::decode_config(url, base64::URL_SAFE_NO_PAD) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let (content, nonce) = input.split_at(input.len() - 12);

    let Ok(nonce) = nonce.try_into() else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(data) = ctx.aes_key.decrypt(nonce, content) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(data) = serde_json::de::from_slice::<Invitation>(&data) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    if data.expire < Utc::now().naive_utc() {
        return StatusCode::GONE.into_response();
    }

    match ctx.accept_permission(&claims.user_id, data.id).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_permission(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i32>() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    match ctx.get_permission(&claims.user_id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.delete_permission(id).await {
        Ok(data) => Json(data).into_response(),
        Err(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
