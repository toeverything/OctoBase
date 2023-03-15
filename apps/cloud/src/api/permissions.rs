use crate::{context::Context, error_status::ErrorStatus};
use axum::{
    extract::Path,
    http::{
        header::{HOST, REFERER},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
    Extension, Json,
};
use cloud_database::{Claims, CreatePermission, PermissionType, UserCred};
use jwst::error;
use jwst_logger::{info, instrument, tracing};
use lettre::message::Mailbox;
use std::sync::Arc;

/// Get workspace's `Members`
/// - Return `Members`.
#[utoipa::path(
    get,
    tag = "Permission",
    context_path = "/api/workspace",
    path = "/{workspace_id}/permission",
    params(
        ("workspace_id", description = "workspace id"),
    )
)]
#[instrument(skip(ctx, claims), fields(user_id = %claims.user.id))]
pub async fn get_members(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<String>,
) -> Response {
    info!("get_members enter");
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

/// Invite workspace members
/// - Return 200 Ok.
#[utoipa::path(
    post,
    tag = "Permission",
    context_path = "/api/workspace",
    path = "/{workspace_id}/permission",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Invite member successfully")
    )
)]
#[instrument(skip(ctx, claims, headers), fields(user_id = %claims.user.id))]
pub async fn invite_member(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Json(data): Json<CreatePermission>,
) -> Response {
    info!("invite_member enter");
    if let Some(site_url) = headers
        .get(REFERER)
        .or_else(|| headers.get(HOST))
        .and_then(|v| v.to_str().ok())
        .and_then(|host| ctx.mail.parse_host(host))
    {
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

        let send_to = Mailbox::new(
            if let UserCred::Registered(user) = user_cred {
                ctx.user_channel
                    .add_user_observe(user.id.clone(), ctx.clone())
                    .await;

                Some(user.id)
            } else {
                None
            },
            addr,
        );

        let metadata = match ctx
            .storage
            .get_workspace(workspace_id.clone())
            .await
            .map(|ws| ws.metadata())
        {
            Ok(metadata) => metadata,
            Err(e) => {
                error!("Failed to send email: {}", e);
                return ErrorStatus::InternalServerError.into_response();
            }
        };

        let Ok(invite_code) = ctx.key.encrypt_aes_base64(permission_id.as_bytes()) else {
            return ErrorStatus::InternalServerError.into_response();
        };

        if let Err(e) = ctx
            .mail
            .send_invite_email(send_to, metadata, site_url, &claims, &invite_code)
            .await
        {
            if let Err(e) = ctx.db.delete_permission(permission_id).await {
                error!("Failed to withdraw permissions: {}", e);
            }
            error!("Failed to send email: {}", e);
            return ErrorStatus::InternalServerError.into_response();
        };

        StatusCode::OK.into_response()
    } else {
        ErrorStatus::BadRequest.into_response()
    }
}

/// Accept invitation
/// - Return permission.
#[utoipa::path(
    post,
    tag = "Permission",
    context_path = "/api/invitation",
    path = "/{path}",
    params(
        ("path", description = "invite code"),
    ),
)]
#[instrument(skip(ctx))]
pub async fn accept_invitation(
    Extension(ctx): Extension<Arc<Context>>,
    Path(url): Path<String>,
) -> Response {
    info!("accept_invitation enter");
    let Ok(data) = ctx.key.decrypt_aes_base64(url) else {
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
        Ok(Some(p)) => {
            if let Some(user_id) = p.user_id.clone() {
                ctx.user_channel.update_user(user_id, ctx.clone());
            };

            Json(p).into_response()
        }
        Ok(None) => ErrorStatus::NotFoundInvitation.into_response(),
        Err(e) => {
            error!("Failed to accept invitation: {}", e);
            ErrorStatus::InternalServerError.into_response()
        }
    }
}

/// Leave workspace
/// - Return 200 ok.
#[utoipa::path(
    delete,
    tag = "Permission",
    context_path = "/api/workspace",
    path = "/{workspace_id}/permission",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Leave workspace successfully")
    )
)]
#[instrument(skip(ctx, claims), fields(user_id = %claims.user.id))]
pub async fn leave_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    info!("leave_workspace enter");
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

/// Remove user from workspace
/// - Return 200 ok.
#[utoipa::path(
    delete,
    tag = "Permission",
    context_path = "/api/permission",
    path = "/{id}",
    params(
        ("id", description = "permission id"),
    ),
    responses(
        (status = 200, description = "Remove member successfully")
    )
)]
#[instrument(skip(ctx, claims), fields(user_id = %claims.user.id))]
pub async fn remove_user(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<String>,
) -> Response {
    info!("remove_user enter");
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
