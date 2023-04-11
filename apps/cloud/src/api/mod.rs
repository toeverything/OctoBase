pub mod blobs;
pub mod permissions;

mod collaboration;
mod common;
mod user_channel;
mod workspace;

pub use collaboration::make_ws_route;

use crate::{
    context::Context, infrastructure::error_status::ErrorStatus, layer::make_firebase_auth_layer,
};
use axum::{
    extract::Query,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put, Router},
    Extension, Json,
};
use chrono::Utc;
use cloud_database::{Claims, MakeToken, RefreshToken, User, UserQuery, UserToken};
use jwst_logger::{error, info, instrument, tracing};
use lib0::any::Any;
use std::sync::Arc;
pub use user_channel::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        query_user,
        make_token,
        common::health_check,
        workspace::get_doc,
        workspace::get_public_doc,
        workspace::get_public_page,
        workspace::get_workspaces,
        workspace::get_workspace_by_id,
        workspace::create_workspace,
        workspace::update_workspace,
        workspace::delete_workspace,
        workspace::search_workspace,
        blobs::get_blob_in_workspace,
        blobs::upload_blob_in_workspace,
        blobs::get_user_resource_usage,
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
    cloud_infra::with_api_doc(route, ApiDoc::openapi(), env!("CARGO_PKG_NAME"))
}

pub fn make_rest_route(ctx: Arc<Context>) -> Router {
    Router::new()
        .route("/healthz", get(common::health_check))
        .route("/user", get(query_user))
        .route("/user/token", post(make_token))
        .route("/invitation/:path", post(permissions::accept_invitation))
        .nest_service("/global/sync", get(global_ws_handler))
        .route("/public/workspace/:id", get(workspace::get_public_doc))
        .route(
            "/public/workspace/:id/:page_id",
            get(workspace::get_public_page),
        )
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
                    get(workspace::get_workspaces).post(workspace::create_workspace),
                )
                .route(
                    "/workspace/:id",
                    get(workspace::get_workspace_by_id)
                        .post(workspace::update_workspace)
                        .delete(workspace::delete_workspace),
                )
                .route("/workspace/:id/doc", get(workspace::get_doc))
                .route("/workspace/:id/search", post(workspace::search_workspace))
                .route(
                    "/workspace/:id/permission",
                    get(permissions::get_members)
                        .post(permissions::invite_member)
                        .delete(permissions::leave_workspace),
                )
                .route("/workspace/:id/blob", put(blobs::upload_blob_in_workspace))
                .route("/permission/:id", delete(permissions::remove_user))
                .route("/resource/usage", get(blobs::get_user_resource_usage))
                .layer(make_firebase_auth_layer(ctx.key.jwt_decode.clone())),
        )
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
#[instrument(skip(ctx))]
pub async fn query_user(
    Extension(ctx): Extension<Arc<Context>>,
    Query(payload): Query<UserQuery>,
) -> Response {
    info!("query_user enter");
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
#[instrument(skip(ctx, payload))] // payload need to be safe
pub async fn make_token(
    Extension(ctx): Extension<Arc<Context>>,
    Json(payload): Json<MakeToken>,
) -> Response {
    info!("make_token enter");
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
        MakeToken::Google { token } => {
            match ctx
                .firebase
                .lock()
                .await
                .decode_google_token(token, ctx.config.refresh_token_expires_in)
                .await
            {
                Ok(claims) => match ctx.db.firebase_user_login(&claims).await {
                    Ok(user) => (Ok(Some(user)), None),
                    Err(e) => {
                        error!("failed to auth: {:?}", e,);
                        return ErrorStatus::InternalServerError.into_response();
                    }
                },
                Err(e) => {
                    error!("failed to check token: {:?}", e);
                    return ErrorStatus::Unauthorized.into_response();
                }
            }
        }
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
                    expires: Utc::now().naive_utc() + ctx.config.refresh_token_expires_in,
                    user_id: user.id.clone(),
                    token_nonce: user.token_nonce.unwrap(),
                };

                let json = serde_json::to_string(&refresh).unwrap();

                ctx.key.encrypt_aes_base64(json.as_bytes()).ok()
            }) else {
                return ErrorStatus::InternalServerError.into_response();
            };

            let claims = Claims {
                exp: Utc::now().naive_utc() + ctx.config.access_token_expires_in,
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

#[cfg(test)]
mod test {
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;
    use cloud_database::{CloudDatabase, CreateUser};
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_query_user() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let new_user = context
            .db
            .create_user(CreateUser {
                avatar_url: Some("xxx".to_string()),
                email: "xxx@xxx.xx".to_string(),
                name: "xxx".to_string(),
                password: "xxx".to_string(),
            })
            .await
            .unwrap();
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let url = format!("/user?email={}&workspace_id=212312", new_user.email);
        let resp = client.get(&url).send().await;

        assert_eq!(resp.status(), StatusCode::OK);
        let resp_text = resp.text().await;
        assert!(resp_text.contains(new_user.id.as_str()));
        assert!(resp_text.contains(new_user.name.as_str()));
        assert!(resp_text.contains(new_user.email.as_str()));

        let resp = client.get("/user").send().await;
        assert_eq!(resp.status().is_client_error(), true);
        let resp = client.get("/user?email=fake_email").send().await;
        assert_eq!(resp.status().is_client_error(), true);
        let resp = client.get("/user?user_name=fake_parameter").send().await;
        assert_eq!(resp.status().is_client_error(), true);
    }

    #[tokio::test]
    async fn test_with_token_expire() {
        std::env::set_var("JWT_REFRESH_TOKEN_EXPIRES_IN", "10");
        std::env::set_var("JWT_ACCESS_TOKEN_EXPIRES_IN", "10");
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

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
        let resp_json: serde_json::Value = resp.json().await;
        let access_token = resp_json["token"].as_str().unwrap().to_string();
        let refresh_token = resp_json["refresh"].as_str().unwrap().to_string();

        let resp = client
            .get("/workspace")
            .header("authorization", format!("{}", access_token))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        std::thread::sleep(std::time::Duration::from_secs(10));
        let body_data = json!({
            "type": "Refresh",
            "token": refresh_token
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let resp = client
            .get("/workspace")
            .header("authorization", format!("{}", access_token))
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_make_token_with_valid_request() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

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
        let refresh_token = resp_json["refresh"].as_str().unwrap().to_string();
        let body_data = json!({
            "type": "Refresh",
            "token": refresh_token,
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_make_token_with_invalid_request() {
        let pool = CloudDatabase::init_pool("sqlite::memory:").await.unwrap();
        let context = Context::new_test_client(pool).await;
        let ctx = Arc::new(context);
        let app = super::make_rest_route(ctx.clone()).layer(Extension(ctx.clone()));

        let client = TestClient::new(app);
        let body_data = json!({
            "type": "DebugCreateUser",
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
        assert_eq!(resp.status().is_client_error(), true);
        let body_data = json!({
            "type": "DebugLoginUser",
            "email": "my_email",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status().is_client_error(), true);
        let body_data = json!({
            "type": "Refresh",
            "token": "my_token",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status().is_client_error(), true);
        let body_data = json!({
            "type": "Google",
            "token": "my_token",
        });
        let body_string = serde_json::to_string(&body_data).unwrap();
        let resp = client
            .post("/user/token")
            .header("Content-Type", "application/json")
            .body(body_string)
            .send()
            .await;
        assert_eq!(resp.status().is_client_error(), true);
    }
}
