use std::sync::Arc;

use axum::{
    body::StreamBody,
    extract::{BodyStream, Path, Query},
    headers::ContentLength,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put, Router},
    Extension, Json, TypedHeader,
};
use chrono::{DateTime, Duration, Utc};
use futures::{future, StreamExt};
use http::{
    header::{
        CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH,
        LAST_MODIFIED,
    },
    HeaderMap, HeaderValue, StatusCode,
};
use jwst::{BlobStorage, DocStorage, Workspace as JWSTWorkspace};
use jwst_storage::{
    Claims, CreatePermission, CreateWorkspace, MakeToken, PermissionType, RefreshToken,
    UpdateWorkspace, UserCred, UserQuery, UserToken, UserWithNonce, WorkspaceMetadata,
    WorkspaceSearchInput,
};
use lettre::{
    message::{Attachment, Mailbox, MultiPart, SinglePart},
    AsyncTransport, Message,
};
use lib0::any::Any;
use mime::APPLICATION_OCTET_STREAM;
use serde::Serialize;
use tower::ServiceBuilder;
use yrs::StateVector;

use crate::{
    context::{Context, ContextRequestError},
    layer::make_firebase_auth_layer,
    utils::URL_SAFE_ENGINE, login::ThirdPartyLogin,
};

mod ws;
pub use ws::*;

pub fn make_rest_route(ctx: Arc<Context>) -> Router {
    Router::new()
        .route("/healthz", get(health_check))
        .route("/user", get(query_user))
        .route("/user/token", post(make_token))
        .route("/blob", put(upload_blob))
        .route("/blob/:name", get(get_blob))
        .route("/invitation/:path", post(accept_invitation))
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
                    get(get_members).post(invite_member).delete(leave_workspace),
                )
                .route("/workspace/:id/doc", get(get_doc))
                .route("/workspace/:id/search", post(search_workspace))
                .route("/workspace/:id/blob", put(upload_blob_in_workspace))
                .route("/workspace/:id/blob/:name", get(get_blob_in_workspace))
                .route("/permission/:id", delete(remove_user))
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

impl Context {
    async fn get_blob(
        &self,
        workspace: Option<String>,
        id: String,
        method: http::Method,
        headers: HeaderMap,
    ) -> Response {
        if let Some(etag) = headers.get(IF_NONE_MATCH).and_then(|h| h.to_str().ok()) {
            if etag == id {
                return StatusCode::NOT_MODIFIED.into_response();
            }
        }

        let Ok(meta) = self.blob.get_metadata(workspace.clone(), id.clone()).await else {
            return StatusCode::NOT_FOUND.into_response()
        };

        if let Some(modified_since) = headers
            .get(IF_MODIFIED_SINCE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok())
        {
            if meta.last_modified <= modified_since.naive_utc() {
                return StatusCode::NOT_MODIFIED.into_response();
            }
        }

        let mut header = HeaderMap::with_capacity(5);
        header.insert(ETAG, HeaderValue::from_str(&id).unwrap());

        header.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(&APPLICATION_OCTET_STREAM.to_string()).unwrap(),
        );

        header.insert(
            LAST_MODIFIED,
            HeaderValue::from_str(&DateTime::<Utc>::from_utc(meta.last_modified, Utc).to_rfc2822())
                .unwrap(),
        );

        header.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&meta.size.to_string()).unwrap(),
        );

        header.insert(
            CACHE_CONTROL,
            HeaderValue::from_str("public, immutable, max-age=31536000").unwrap(),
        );

        if method == http::Method::HEAD {
            return header.into_response();
        };

        let Ok(file) = self.blob.get_blob(workspace, id).await else {
            return StatusCode::NOT_FOUND.into_response()
        };

        (header, StreamBody::new(file)).into_response()
    }

    async fn upload_blob(&self, stream: BodyStream, workspace: Option<i64>) -> Response {
        // TODO: cancel
        let mut has_error = false;
        let stream = stream
            .take_while(|x| {
                has_error = x.is_err();
                future::ready(x.is_ok())
            })
            .filter_map(|data| future::ready(data.ok()));
        let workspace = workspace.map(|id| id.to_string());

        if let Ok(id) = self.blob.put_blob(workspace.clone(), stream).await {
            if has_error {
                let _ = self.blob.delete_blob(workspace, id).await;
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            } else {
                id.into_response()
            }
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_blob(
    Extension(ctx): Extension<Arc<Context>>,
    Path(id): Path<String>,
    method: http::Method,
    headers: HeaderMap,
) -> Response {
    ctx.get_blob(None, id, method, headers).await
}

async fn upload_blob(
    Extension(ctx): Extension<Arc<Context>>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    if length.0 > 500 * 1024 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }

    ctx.upload_blob(stream, None).await
}

async fn get_workspace_by_id(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.get_permission(claims.user.id, id).await {
        Ok(Some(_)) => (),
        Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.get_workspace_by_id(id).await {
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
                t.set_metadata(
                    "avatar",
                    Any::String(payload.avatar.clone().into_boxed_str()),
                );
            });
            doc
        };
        if let Err(_) = ctx.doc.storage.write_doc(data.id, doc.doc()).await {
            StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };

        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn update_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateWorkspace>,
) -> Response {
    match ctx.db.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.update_workspace(id, payload).await {
        Ok(Some(data)) => Json(data).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.is_owner() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    match ctx.db.delete_workspace(id).await {
        Ok(true) => {
            let _ = ctx.blob.delete_workspace(id.to_string()).await;
            StatusCode::OK.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path((workspace, id)): Path<(i64, String)>,
    method: http::Method,
    headers: HeaderMap,
) -> Response {
    match ctx.db.can_read_workspace(claims.user.id, workspace).await {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let workspace = workspace.to_string();

    ctx.get_blob(Some(workspace), id, method, headers).await
}

async fn get_doc(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace_id): Path<i64>,
) -> Response {
    match ctx
        .db
        .can_read_workspace(claims.user.id, workspace_id)
        .await
    {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    if let Some(doc) = ctx.doc.try_get_workspace(workspace_id) {
        return doc
            .read()
            .await
            .doc()
            .encode_state_as_update_v1(&StateVector::default())
            .into_response();
    }

    match ctx.doc.storage.get(workspace_id).await {
        Ok(doc) => doc
            .encode_state_as_update_v1(&StateVector::default())
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn upload_blob_in_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(workspace): Path<i64>,
    TypedHeader(length): TypedHeader<ContentLength>,
    stream: BodyStream,
) -> Response {
    if length.0 > 10 * 1024 * 1024 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }

    match ctx.db.can_read_workspace(claims.user.id, workspace).await {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    ctx.upload_blob(stream, Some(workspace)).await
}

/// Resolves to [WorkspaceSearchResults]
async fn search_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
    Json(payload): Json<WorkspaceSearchInput>,
) -> Response {
    match ctx.db.can_read_workspace(claims.user.id, id).await {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let search_results = match ctx.search_workspace(id, &payload.query).await {
        Ok(results) => results,
        Err(err) => return err.into_response(),
    };

    Json(search_results).into_response()
}

async fn get_members(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Ok(members) = ctx.db.get_workspace_members(id).await {
        Json(members).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn make_invite_email(
    ctx: &Context,
    id: i64,
    claims: &Claims,
    invite_code: &str,
) -> Option<(String, MultiPart)> {
    let metadata = {
        let ws = ctx.doc.get_workspace(id).await?;

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

async fn invite_member(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
    Json(data): Json<CreatePermission>,
) -> Response {
    match ctx.db.get_permission(claims.user.id, id).await {
        Ok(Some(p)) if p.can_admin() => (),
        Ok(_) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let Ok(addr) = data.email.clone().parse() else {
        return StatusCode::BAD_REQUEST.into_response()
    };

    let (permission_id, user_cred) = match ctx
        .db
        .create_permission(&data.email, id, PermissionType::Write)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::CONFLICT.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let encrypted = ctx.encrypt_aes(&permission_id.to_le_bytes()[..]);

    let invite_code = base64::encode_engine(encrypted, &URL_SAFE_ENGINE);

    let mailbox = Mailbox::new(
        match user_cred {
            UserCred::Registered(user) => Some(user.name),
            UserCred::UnRegistered { .. } => None,
        },
        addr,
    );

    let Some((title, msg_body)) = make_invite_email(&ctx, id, &claims, &invite_code).await else {
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

async fn accept_invitation(
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

async fn leave_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.delete_permission_by_query(claims.user.id, id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn remove_user(
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
