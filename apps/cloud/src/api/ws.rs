use super::*;
use axum::{
    extract::{
        ws::{close_code, CloseFrame, Message, WebSocketUpgrade},
        Path,
    },
    response::Response,
};
use base64::Engine;
use jwst_rpc::handle_socket;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize)]
pub struct WebSocketAuthentication {
    protocol: String,
}

pub fn make_ws_route() -> Router {
    Router::new().route("/:id", get(ws_handler))
}

#[derive(Deserialize)]
struct Param {
    token: String,
}

async fn ws_handler(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    Query(Param { token }): Query<Param>,
    ws: WebSocketUpgrade,
) -> Response {
    let user: Option<RefreshToken> = URL_SAFE_ENGINE
        .decode(token)
        .ok()
        .and_then(|byte| match ctx.key.decrypt_aes(byte) {
            Ok(data) => data,
            Err(_) => None,
        })
        .and_then(|data| serde_json::from_slice(&data).ok());

    let user = if let Some(user) = user {
        if let Ok(true) = ctx.db.verify_refresh_token(&user).await {
            Some(user.user_id)
        } else {
            None
        }
    } else {
        None
    };

    ws.protocols(["AFFiNE"])
        .on_upgrade(move |mut socket| async move {
            let user_id = if let Some(user_id) = user {
                if let Ok(true) = ctx
                    .db
                    .can_read_workspace(user_id.clone(), workspace.clone())
                    .await
                {
                    Some(user_id)
                } else {
                    None
                }
            } else {
                None
            };
            let user_id = if let Some(user_id) = user_id {
                user_id
            } else {
                let _ = socket
                    .send(ws::Message::Close(Some(CloseFrame {
                        code: close_code::POLICY,
                        reason: "Unauthorized".into(),
                    })))
                    .await;
                return;
            };

            handle_socket(socket, workspace, ctx.clone(), user_id).await
        })
}
