use crate::{
    context::Context,
    error_status::ErrorStatus,
    utils::{Engine, URL_SAFE_ENGINE},
};
use axum::{
    extract::Path,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::prelude::*;
use cloud_database::{Claims, CreatePermission, PermissionType, UserCred};
use http::StatusCode;
use jwst::error;
use lettre::{
    message::{Mailbox, MultiPart, SinglePart},
    AsyncTransport, Message,
};
use serde::Serialize;
use std::sync::Arc;

pub async fn get_members(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    match ctx
        .db
        .get_permission(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    };

    match ctx.db.get_workspace_members(workspace_id).await {
        Ok(members) => Json(members).into_response(),
        Err(e) => {
            error!("Failed to get workspace members: {}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

async fn make_invite_email(
    ctx: &Context,
    workspace_id: String,
    claims: &Claims,
    invite_code: &str,
) -> Option<(String, MultiPart)> {
    let metadata = {
        let ws = ctx.storage.get_workspace(workspace_id).await.ok()?;
        let ws = ws.read().await;
        ws.metadata()
    };

    // let mut file = ctx.blob.get_blob(None, metadata.avatar).await.ok()?;

    // let mut file_content = Vec::new();
    // while let Some(chunk) = file.next().await {
    //     file_content.extend(chunk.ok()?);
    // }

    // let workspace_avatar = lettre::message::Body::new(file_content);

    #[derive(Serialize)]
    struct Title {
        inviter_name: String,
        workspace_name: String,
    }

    let title = ctx
        .mail
        .template
        .render(
            "MAIL_INVITE_TITLE",
            &Title {
                inviter_name: claims.user.name.clone(),
                workspace_name: metadata.name.clone().unwrap_or_default(),
            },
        )
        .ok()?;

    #[derive(Serialize)]
    struct Content {
        inviter_name: String,
        site_url: String,
        avatar_url: String,
        workspace_name: String,
        invite_code: String,
        current_year: i32,
    }
    let dt = Utc::now();
    let content = ctx
        .mail
        .template
        .render(
            "MAIL_INVITE_CONTENT",
            &Content {
                inviter_name: claims.user.name.clone(),
                site_url: ctx.site_url.clone(),
                avatar_url: claims.user.avatar_url.to_owned().unwrap_or("".to_string()),
                workspace_name: metadata.name.unwrap_or_default(),
                invite_code: invite_code.to_string(),
                current_year: dt.year(),
            },
        )
        .ok()?;

    let msg_body = MultiPart::mixed().multipart(
        MultiPart::mixed().multipart(MultiPart::related().singlepart(SinglePart::html(content))),
    );

    Some((title, msg_body))
}

pub async fn invite_member(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
    Json(data): Json<CreatePermission>,
) -> Response {
    match ctx
        .db
        .get_permission(claims.user.id.clone(), workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    };

    let Ok(addr) = data.email.clone().parse() else {
        return ErrorStatus::BadRequest.into_response()
    };

    let (permission_id, user_cred) = match ctx
        .db
        .create_permission(&data.email, workspace_id.clone(), PermissionType::Write)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return ErrorStatus::ConflictInvitation.into_response(),
        Err(e) => {
            error!("Failed to create permission: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    };

    let invite_user = user_cred.clone();
    let invite_user_id = match invite_user {
        UserCred::Registered(user) => Some(user.id),
        UserCred::UnRegistered { .. } => None,
    };
    if invite_user_id.is_some() {
        ctx.user_channel
            .add_user_observe(invite_user_id.unwrap(), ctx.clone())
            .await;
    }

    let encrypted = ctx.encrypt_aes(permission_id.as_bytes());

    let invite_code = URL_SAFE_ENGINE.encode(encrypted);

    let mailbox = Mailbox::new(
        match user_cred {
            UserCred::Registered(user) => Some(user.name),
            UserCred::UnRegistered { .. } => None,
        },
        addr,
    );

    let Some((title, msg_body)) = make_invite_email(&ctx, workspace_id, &claims, &invite_code).await else {
        let _ = ctx.db.delete_permission(permission_id);
        return ErrorStatus::InternalServerError.into_response();
    };

    let email = Message::builder()
        .from(ctx.mail.mail_box.clone())
        .to(mailbox)
        .subject(title)
        .multipart(msg_body)
        .unwrap();

    match ctx.mail.client.send(email.clone()).await {
        Ok(_) => {}
        // TODO: https://github.com/lettre/lettre/issues/743
        Err(e) if e.is_response() => {
            if let Err(e) = ctx.mail.client.send(email).await {
                if let Err(e) = ctx.db.delete_permission(permission_id).await {
                    error!("Failed to withdraw permissions: {}", e);
                }
                error!("Failed to send email: {}", e);
                return ErrorStatus::InternalServerError.into_response();
            }
        }
        Err(e) => {
            if let Err(e) = ctx.db.delete_permission(permission_id).await {
                error!("Failed to withdraw permissions: {}", e);
            }
            error!("Failed to send email: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    };

    StatusCode::OK.into_response()
}

pub async fn accept_invitation(
    Extension(ctx): Extension<Arc<Context>>,
    Path(url): Path<String>,
) -> Response {
    let Ok(input) = URL_SAFE_ENGINE.decode(url) else {
        return ErrorStatus::BadRequest.into_response();
    };

    let data = match ctx.decrypt_aes(input.clone()) {
        Ok(data) => data,
        Err(_) => return ErrorStatus::BadRequest.into_response(),
    };

    let Some(data) = data else {
        return ErrorStatus::BadRequest.into_response();
    };

    // let Ok(data) = TryInto::<[u8; 8]>::try_into(data) else {
    //     return ErrorStatus::BadRequest.into_response();
    // };

    match ctx
        .db
        .accept_permission(String::from_utf8(data).unwrap())
        .await
    {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => ErrorStatus::NotFoundInvitation.into_response(),
        Err(e) => {
            error!("Failed to accept invitation: {}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

pub async fn leave_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    match ctx
        .db
        .delete_permission_by_query(claims.user.id.clone(), id.clone())
        .await
    {
        Ok(true) => {
            ctx.user_channel
                .update_user(claims.user.id.clone(), ctx.clone());
            ctx.close_websocket(id.clone(), claims.user.id.clone())
                .await;

            StatusCode::OK.into_response()
        }
        Ok(false) => StatusCode::OK.into_response(),
        Err(e) => {
            error!("Failed to leave workspace: {}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

pub async fn remove_user(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    match ctx
        .db
        .get_permission_by_permission_id(claims.user.id.clone(), id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return ErrorStatus::Forbidden.into_response(),
        Err(e) => {
            error!("Failed to get permission: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        }
    };

    let permission_model = ctx
        .db
        .get_permission_by_id(id.clone())
        .await
        .unwrap()
        .unwrap();
    match ctx.db.delete_permission(id).await {
        Ok(true) => {
            if let Some(user_id) = permission_model.user_id {
                ctx.user_channel.update_user(user_id.clone(), ctx.clone());
                ctx.close_websocket(permission_model.workspace_id.clone(), user_id.clone())
                    .await;
            };
            StatusCode::OK.into_response()
        }
        Ok(false) => {
            if let Some(user_id) = permission_model.user_id {
                ctx.user_channel.update_user(user_id.clone(), ctx.clone());
                ctx.close_websocket(permission_model.workspace_id.clone(), user_id.clone())
                    .await;
            };
            StatusCode::OK.into_response()
        }
        Err(e) => {
            error!("Failed to remove user: {}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}
