use super::*;
use axum::{
    extract::{
        ws::{close_code, CloseFrame, Message, WebSocketUpgrade},
        Path,
    },
    response::Response,
};
use jsonwebtoken::{decode, Validation};
use jwst_rpc::{handle_connector, socket_connector};
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
    let key = ctx.key.jwt_decode.clone();
    let user = match decode::<Claims>(&token, &key, &Validation::default()).map(|d| d.claims) {
        Ok(claims) => Some(claims.user.id),
        Err(_) => None,
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
                    .send(collaboration::Message::Close(Some(CloseFrame {
                        code: close_code::POLICY,
                        reason: "Unauthorized".into(),
                    })))
                    .await;
                return;
            };

            handle_connector(ctx.clone(), workspace.clone(), user_id, move || {
                socket_connector(socket, &workspace)
            })
            .await
        })
}
