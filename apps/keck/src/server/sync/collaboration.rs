use super::*;
use axum::{
    extract::{ws::WebSocketUpgrade, Path},
    response::Response,
    Json,
};
use jwst_logger::error;
use jwst_rpc::handle_socket;
use serde::Serialize;
use std::sync::Arc;
use tokio::task::spawn_blocking;

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub async fn auth_handler(Path(workspace_id): Path<String>) -> Json<WebSocketAuthentication> {
    info!("auth: {}", workspace_id);
    Json(WebSocketAuthentication {
        protocol: "AFFiNE".to_owned(),
    })
}

pub async fn upgrade_handler(
    Extension(context): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let identifier = Uuid::new_v4().to_string();
    ws.protocols(["AFFiNE"]).on_upgrade(|socket| async move {
        if let Err(e) = spawn_blocking(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                handle_socket(socket, workspace, context.clone(), identifier).await
            });
        })
        .await
        {
            error!("sync thread error: {:?}", e);
        }
    })
}
