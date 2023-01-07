use axum::{
    extract::Path,
    response::{IntoResponse, Response},
    Extension, Json,
};

use crate::{context::Context, utils::URL_SAFE_ENGINE};
use futures::StreamExt;
use http::StatusCode;
use jwst::{BlobStorage, Workspace as JWSTWorkspace};
use jwst_storage::{Claims, CreatePermission, PermissionType, UserCred, WorkspaceMetadata};
use lettre::{
    message::{Attachment, Mailbox, MultiPart, SinglePart},
    AsyncTransport, Message,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn get_members(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    match ctx
        .db
        .get_permission(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Ok(members) = ctx.db.get_workspace_members(workspace_id).await {
        Json(members).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn make_invite_email(
    ctx: &Context,
    workspace_id: String,
    claims: &Claims,
    invite_code: &str,
) -> Option<(String, MultiPart)> {
    let metadata = {
        let workspace_id = workspace_id.to_string();
        let ws = ctx
            .docs
            .create_doc(&workspace_id)
            .await
            .map(|f| {
                Arc::new(RwLock::new(JWSTWorkspace::from_doc(
                    f,
                    workspace_id.to_string(),
                )))
            })
            .ok()?;

        let ws = ws.read().await;

        WorkspaceMetadata::parse(ws.metadata())?
    };

    let mut file = ctx.blob.get_blob(None, metadata.avatar).await.ok()?;

    let mut file_content = Vec::new();
    while let Some(chunk) = file.next().await {
        file_content.extend(chunk.ok()?);
    }

    let workspace_avatar = lettre::message::Body::new(file_content);

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
                workspace_name: metadata.name.clone(),
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
    }

    let content = ctx
        .mail
        .template
        .render(
            "MAIL_INVITE_CONTENT",
            &Content {
                inviter_name: claims.user.name.clone(),
                site_url: ctx.site_url.clone(),
                avatar_url: claims.user.avatar_url.to_owned().unwrap_or("".to_string()),
                workspace_name: metadata.name,
                invite_code: invite_code.to_string(),
            },
        )
        .ok()?;

    let msg_body = MultiPart::mixed().multipart(
        MultiPart::mixed().multipart(
            MultiPart::related()
                .singlepart(SinglePart::html(content))
                .singlepart(
                    Attachment::new_inline("avatar".to_string())
                        .body(workspace_avatar, "image/png".parse().unwrap()),
                ),
        ),
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
        .get_permission(claims.user.id, workspace_id.clone())
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let Ok(addr) = data.email.clone().parse() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    let (permission_id, user_cred) = match ctx
        .db
        .create_permission(&data.email, workspace_id.clone(), PermissionType::Write)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::CONFLICT.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let invite_user = user_cred.clone();
    let invite_user_id = match invite_user {
        UserCred::Registered(user) => Some(user.id),
        UserCred::UnRegistered { .. } => None,
    };
    if !invite_user_id.is_none() {
        ctx.user_channel
            .add_user_observe(invite_user_id.unwrap(), ctx.clone())
            .await;
    }

    let encrypted = ctx.encrypt_aes(&permission_id.to_le_bytes()[..]);

    let invite_code = base64::encode_engine(encrypted, &URL_SAFE_ENGINE);

    let mailbox = Mailbox::new(
        match user_cred {
            UserCred::Registered(user) => Some(user.name),
            UserCred::UnRegistered { .. } => None,
        },
        addr,
    );

    let Some((title, msg_body)) = make_invite_email(&ctx, workspace_id, &claims, &invite_code).await else {
        let _ = ctx.db.delete_permission(permission_id);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
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
            if let Err(_) = ctx.mail.client.send(email).await {
                let _ = ctx.db.delete_permission(permission_id);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
        Err(_) => {
            let _ = ctx.db.delete_permission(permission_id).await;
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    StatusCode::OK.into_response()
}

pub async fn accept_invitation(
    Extension(ctx): Extension<Arc<Context>>,
    Path(url): Path<String>,
) -> Response {
    let Ok(input) = base64::decode_engine(url, &URL_SAFE_ENGINE) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Some(data) = ctx.decrypt_aes(input) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(data) = TryInto::<[u8; 8]>::try_into(data) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match ctx.db.accept_permission(i64::from_le_bytes(data)).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn leave_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    match ctx.db.delete_permission_by_query(claims.user.id, id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn remove_user(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx
        .db
        .get_permission_by_permission_id(claims.user.id, id)
        .await
    {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.delete_permission(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
