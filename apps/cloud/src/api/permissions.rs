use crate::{context::Context, infrastructure::error_status::ErrorStatus};
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
use image::ImageOutputFormat;
use jwst::{error, BlobStorage};
use jwst_logger::{info, instrument, tracing};
use lettre::message::Mailbox;
use std::io::Cursor;
use std::sync::Arc;
/// Get workspace's `Members`
/// - Return 200 ok and `Members`.
/// - Return 400 if request parameter error.
/// - Return 403 if user do not have permission.
/// - Return 500 if server error.
#[utoipa::path(
    get,
    tag = "Permission",
    context_path = "/api/workspace",
    path = "/{workspace_id}/permission",
    params(
        ("workspace_id", description = "workspace id"),
    ),
    responses(
        (status = 200, description = "Return member", body = [Vec<Member>],
        example=json!([{
        "id": "xxxxx",
        "user":  {
            "id": "xxx",
            "name": "xxx",
            "email": "xxx@xxx.xx",
            "avatar_url": "xxx",
            "created_at": "2023-03-16T08:51:08" },
        "accepted": "true",
        "type": "Owner",
        "created_at": "2023-03-16T08:51:08"
        }])

       ),
        (status = 400, description = "Request parameter error."),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 500, description = "Server error, please try again later.")
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
    request_body(content = CreatePermission, description = "Request body for invite member",content_type = "application/json",example = json!({
        "email": "toeverything@toeverything.info"
    }
    )),
    responses(
        (status = 200, description = "Invite member successfully"),
        (status = 400, description = "Request parameter error."),
        (status = 409, description = "Invitation failed."),
        (status = 500, description = "Server error, please try again later.")
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
    let is_test_email = data.clone().email.contains("example@toeverything.info");
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
                if let Err(e) = ctx.db.delete_permission(permission_id).await {
                    error!("Failed to withdraw permissions: {}", e);
                }
                error!("Failed to send email: {}", e);
                return ErrorStatus::InternalServerError.into_response();
            }
        };

        let workspace_avatar_data = match metadata.avatar.clone() {
            Some(avatar) => match ctx
                .storage
                .blobs()
                .get_blob(Some(workspace_id.clone()), avatar.clone(), None)
                .await
            {
                Ok(avatar) => {
                    info!("avatar: Done");
                    avatar
                }
                Err(e) => {
                    error!("Failed to get workspace avatar: {}", e);
                    Vec::new()
                }
            },
            None => {
                info!("avatar: None");
                Vec::new()
            }
        };

        fn convert_to_jpeg(data: &[u8]) -> Result<Vec<u8>, image::ImageError> {
            let image = image::load_from_memory(data)?;
            let mut buffer = Cursor::new(Vec::new());
            image.write_to(&mut buffer, ImageOutputFormat::Jpeg(80))?;
            Ok(buffer.into_inner())
        }
        let workspace_avatar_data = match convert_to_jpeg(&workspace_avatar_data) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to convert avatar to jpeg: {}", e);
                Vec::new()
            }
        };
        let Ok(invite_code) = ctx.key.encrypt_aes_base64(permission_id.as_bytes()) else {
            return ErrorStatus::InternalServerError.into_response();
        };
        if !is_test_email {
            if let Err(e) = ctx
                .mail
                .send_invite_email(
                    send_to,
                    metadata,
                    site_url,
                    &claims,
                    &invite_code,
                    workspace_avatar_data,
                )
                .await
            {
                if let Err(e) = ctx.db.delete_permission(permission_id).await {
                    error!("Failed to withdraw permissions: {}", e);
                }
                error!("Failed to send email: {}", e);
                return ErrorStatus::InternalServerError.into_response();
            };
        } else if is_test_email {
            return Json(&invite_code).into_response();
        }

        StatusCode::OK.into_response()
    } else {
        ErrorStatus::BadRequest.into_response()
    }
}

/// Accept invitation
/// - Return 200 ok and permission.
/// - Return 400 bad request.
/// - Return 401 unauthorized.
/// - Return 404 invitation link has expired.
/// - Return 500 server error.
#[utoipa::path(
    post,
    tag = "Permission",
    context_path = "/api/invitation",
    path = "/{path}",
    params(
        ("path", description = "invite code"),
    ),
    responses(
        (status = 200, description = "Return permission", body = Permission,
        example=json!({
            "id": "xxxxx",
            "type": "Admin",
            "workspace_id": "xxxx",
            "user_id": ("xxx"),
            "user_email": ("xxx2@xxx.xx"),
            "accepted": "true",
            "created_at": "2023-03-16T09:29:28" }
        )),
        (status = 400, description = "Request parameter error."),
        (status = 401, description = "Unauthorized."),
        (status = 404, description = "Invitation link has expired."),
        (status = 500, description = "Server error, please try again later.")
    )
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
        (status = 200, description = "Leave workspace successfully"),
        (status = 500, description = "Server error, please try again later.")
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
/// - Return 200 Ok.
/// - Return 403 Sorry, you do not have permission.
/// - Return 500 server error.
#[utoipa::path(
    delete,
    tag = "Permission",
    context_path = "/api/permission",
    path = "/{id}",
    params(
        ("id", description = "permission id"),
    ),
    responses(
        (status = 200, description = "Remove member successfully"),
        (status = 403, description = "Sorry, you do not have permission."),
        (status = 500, description = "Server error, please try again later.")
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

    let permission_result = match ctx.db.get_permission_by_id(id.clone()).await {
        Ok(result) => result,
        Err(_) => return ErrorStatus::InternalServerError.into_response(),
    };
    let permission_model = match permission_result {
        Some(model) => model,
        None => return ErrorStatus::InternalServerError.into_response(),
    };

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

#[cfg(test)]
mod test {
    use super::{super::make_rest_route, *};
    use axum::body::Body;
    use axum_test_helper::TestClient;
    use bytes::Bytes;
    use cloud_database::CloudDatabase;
    use futures::stream;
    use serde_json::json;

    #[tokio::test]
    async fn test_get_member() {
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
        let url = format!("/workspace/{}/permission", workspace_id);
        let resp = client
            .get(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text).unwrap();
        let first_object = resp_json[0].as_object().unwrap();
        let permission = first_object["type"].as_i64().unwrap();
        assert_eq!(permission, 99);
        let resp = client
            .get("/workspace/mock_id/permission")
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_invite_member() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "my_username",
            "avatar_url": "my_avatar_url",
            "email": "api_test@toeverything.info",
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
            "email": "api_test@toeverything.info",
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
        let url = format!("/workspace/{}/permission", workspace_id.clone());
        let referer_url = format!(
            "https://nightly.affine.pro/workspace/{}",
            workspace_id.clone()
        );
        let body_data = json!({
            "email": "example@toeverything.info",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .header("referer", &referer_url)
            .body(body_string.clone())
            .send()
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .header("referer", &referer_url)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp = client
            .post("/workspace/mock_id/permission")
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .header("referer", &referer_url)
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_accept_invitation() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        // create user 1th
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "my_username",
            "avatar_url": "my_avatar_url",
            "email": "api_test@toeverything.info",
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
        // login user 1th
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "api_test@toeverything.info",
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
        // create workspace
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
        let url = format!("/workspace/{}/permission", workspace_id.clone());
        let workspace_url = format!("/workspace/{}", workspace_id);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let referer_url = format!(
            "https://nightly.affine.pro/workspace/{}",
            workspace_id.clone()
        );
        let body_data = json!({
            "email": "example@toeverything.info",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        // create invitation
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .header("referer", &referer_url)
            .body(body_string.clone())
            .send()
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        let invite_code = resp_text.trim_matches('\"').to_string();
        // create user 2th
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "new_username",
            "avatar_url": "new_avatar_url",
            "email": "example@toeverything.info",
            "password": "new_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        // login user 2th
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "example@toeverything.info",
            "password": "new_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let access_token = resp_json["token"].as_str().unwrap().to_string();
        let url = format!("/invitation/{}", invite_code);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        // accept invitation
        let resp = client.post(&url).send().await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_leave_workspace() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        // create user 1th
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "my_username",
            "avatar_url": "my_avatar_url",
            "email": "api_test@toeverything.info",
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
        // login user 1th
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "api_test@toeverything.info",
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
        // create workspace
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
        let url = format!("/workspace/{}/permission", workspace_id.clone());
        let workspace_url = format!("/workspace/{}", workspace_id);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let referer_url = format!(
            "https://nightly.affine.pro/workspace/{}",
            workspace_id.clone()
        );
        let body_data = json!({
            "email": "example@toeverything.info",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        // create invitation
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .header("Content-Type", "application/json")
            .header("referer", &referer_url)
            .body(body_string.clone())
            .send()
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        let invite_code = resp_text.trim_matches('\"').to_string();
        // create user 2th
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "new_username",
            "avatar_url": "new_avatar_url",
            "email": "example@toeverything.info",
            "password": "new_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        // login user 2th
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "example@toeverything.info",
            "password": "new_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let access_token = resp_json["token"].as_str().unwrap().to_string();
        let url = format!("/invitation/{}", invite_code);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        // accept invitation
        let resp = client.post(&url).send().await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        //leave workspace
        let url = format!("/workspace/{}/permission", workspace_id.clone());
        let resp = client
            .delete(&url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_remove_user() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        // create user 1th
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "my_username",
            "avatar_url": "my_avatar_url",
            "email": "api_test@toeverything.info",
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
        // login user 1th
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "api_test@toeverything.info",
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
        let access_token_owner = resp_json["token"].as_str().unwrap().to_string();
        let test_data: Vec<u8> = (0..=255).collect();
        let test_data_len = test_data.len();
        let test_data_stream = stream::iter(
            test_data
                .into_iter()
                .map(|byte| Ok::<_, std::io::Error>(Bytes::from(vec![byte]))),
        );
        let body_stream = Body::wrap_stream(test_data_stream.clone());
        // create workspace
        let resp = client
            .post("/workspace")
            .header("Content-Length", test_data_len.to_string())
            .header("authorization", format!("{}", access_token_owner.clone()))
            .body(body_stream)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let workspace_id = resp_json["id"].as_str().unwrap().to_string();
        let url = format!("/workspace/{}/permission", workspace_id.clone());
        let workspace_url = format!("/workspace/{}", workspace_id);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token_owner.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let referer_url = format!(
            "https://nightly.affine.pro/workspace/{}",
            workspace_id.clone()
        );
        let body_data = json!({
            "email": "example@toeverything.info",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        // create invitation
        let resp = client
            .post(&url)
            .header("authorization", format!("{}", access_token_owner.clone()))
            .header("Content-Type", "application/json")
            .header("referer", &referer_url)
            .body(body_string.clone())
            .send()
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        let invite_code = resp_text.trim_matches('\"').to_string();
        // create user 2th
        let body_data = json!({
            "type": "DebugCreateUser",
            "name": "new_username",
            "avatar_url": "new_avatar_url",
            "email": "example@toeverything.info",
            "password": "new_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        // login user 2th
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "example@toeverything.info",
            "password": "new_password",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string.clone())
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let access_token_member = resp_json["token"].as_str().unwrap().to_string();
        let url = format!("/invitation/{}", invite_code);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token_member.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        // accept invitation
        let resp = client.post(&url).send().await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp_json: serde_json::Value = resp.json().await;
        let permission_id = resp_json["id"].as_str().unwrap().to_string();

        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token_member.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        //leave workspace
        let url = format!("/permission/{}", permission_id.clone());
        let resp = client
            .delete(&url)
            .header("authorization", format!("{}", access_token_member.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let resp = client
            .delete(&url)
            .header("authorization", format!("{}", access_token_owner.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = client
            .get(&workspace_url)
            .header("authorization", format!("{}", access_token_member.clone()))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
