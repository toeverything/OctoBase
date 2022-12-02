use std::sync::Arc;

use axum::{
    extract::Path,
    response::{IntoResponse, Response},
    routing::{delete, get, post, Router},
    Extension, Json,
};
use hmac::Mac;
use http::StatusCode;
use tower::ServiceBuilder;
use tracing::info;

use crate::{
    context::Context,
    database::CreatePrivateWorkspaceError,
    layer::make_firebase_auth_layer,
    model::{Claims, CreatePermission, CreateWorkspace, UpdateWorkspace},
    utils::HmacSha256,
};

mod ws;

pub fn make_rest_route() -> Router {
    Router::new()
        .route("/workspace", get(get_workspaces).post(create_workspace))
        .route("/workspace/private", post(create_private_workspace))
        .route(
            "/workspace/:id",
            get(get_workspace_by_id).post(update_workspace),
        )
        .route("/share_url/:path", get(access_share_url))
        .route(
            "/workspace/:id/share_url",
            post(share_workspace).delete(stop_share_workspace),
        )
        .route("/workspace/:id/permission", delete(delete_permission))
        .layer(ServiceBuilder::new().layer(make_firebase_auth_layer()))
}

async fn get_workspaces(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    if let Ok(data) = ctx.get_workspace(&claims.user_id).await {
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

    match ctx.can_read_workspace(&claims.user_id, id).await {
        Ok(true) => (),
        Ok(false) => return StatusCode::FORBIDDEN.into_response(),
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
    match ctx.create_normal_workspace(&claims.user_id, payload).await {
        Ok(data) => Json(data).into_response(),
        Err(e) => {
            info!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn create_private_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    match ctx.create_private_workspace(&claims.user_id).await {
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

async fn access_share_url(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Path(url): Path<String>,
) -> Response {
    let share_url = match ctx.get_workspace_by_share_url(url).await {
        Ok(share_url) => share_url,
        Err(sqlx::Error::RowNotFound) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Ok(_) = ctx
        .create_permission(
            &claims.user_id,
            CreatePermission {
                workspace_id: share_url.workspace_id,
            },
        )
        .await
    {
        StatusCode::OK.into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn share_workspace(
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

    let mut mac = HmacSha256::new_from_slice(&ctx.hash_key).unwrap();
    mac.update(&id.to_be_bytes());

    let result = mac.finalize().into_bytes();
    let result = base64::encode_config(result, base64::URL_SAFE_NO_PAD);

    match ctx.create_share_url(id, result).await {
        Ok(url) => Json(url).into_response(),
        Err(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn stop_share_workspace(
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

    match ctx.delete_share_url(id).await {
        Ok(url) => Json(url).into_response(),
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

    match ctx.delete_permission(&claims.user_id, id).await {
        Ok(data) => Json(data).into_response(),
        Err(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
