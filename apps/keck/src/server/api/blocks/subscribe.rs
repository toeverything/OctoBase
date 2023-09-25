use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example=json!({
    "endpoint": "http://localhost:3000/api/hook"
}))]
pub struct SubscribeWorkspace {
    #[serde(rename = "endpoint")]
    pub endpoint: String,
}

/// Register a webhook for all block changes from all workspace changes
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/subscribe",
    path = "",
    request_body(
        content_type = "application/json",
        content = SubscribeWorkspace,
        description = "Provide endpoint of webhook server",
    ),
    responses(
        (status = 200, description = "Subscribe workspace succeed"),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn subscribe_workspace(
    Extension(context): Extension<Arc<Context>>,
    Json(payload): Json<SubscribeWorkspace>,
) -> Response {
    info!("subscribe all workspaces, hook endpoint: {}", payload.endpoint);
    context.set_webhook(payload.endpoint);
    info!("successfully subscribed all workspaces");
    StatusCode::OK.into_response()
}

/// The webhook receiver for debug history subscribe feature
#[utoipa::path(
    post,
    tag = "Workspace",
    context_path = "/api/hook",
    path = "",
    request_body(
        content_type = "application/json",
        content = [History],
        description = "Histories of block changes"
    ),
    responses(
        (status = 200, description = "Histories received"),
    )
)]
pub async fn subscribe_test_hook(Json(payload): Json<Vec<BlockHistory>>) -> Response {
    info!("webhook receive {} histories", payload.len());
    StatusCode::OK.into_response()
}
