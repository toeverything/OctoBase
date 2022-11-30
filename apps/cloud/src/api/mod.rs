use std::sync::Arc;

use axum::{
    response::{IntoResponse, Response},
    routing::{get, Router},
    Extension, Json,
};
use http::StatusCode;
use tower::ServiceBuilder;

use crate::{
    context::Context,
    layer::make_firebase_auth_layer,
    model::{Claims, CreateWorkspace},
};

mod ws;

pub fn make_rest_route() -> Router {
    Router::new()
        .route("/workspace", get(get_workspace).post(create_workspace))
        .layer(ServiceBuilder::new().layer(make_firebase_auth_layer()))
}

async fn get_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
) -> Response {
    if let Ok(data) = ctx.get_workspace(&claims.user_id).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn create_workspace(
    Extension(ctx): Extension<Arc<Context>>,
    Extension(claims): Extension<Arc<Claims>>,
    Json(payload): Json<CreateWorkspace>,
) -> Response {
    if let Ok(data) = ctx.create_workspace(&claims.user_id, payload).await {
        Json(data).into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
