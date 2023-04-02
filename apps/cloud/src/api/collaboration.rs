use super::*;
use axum::{
    extract::{ws::WebSocketUpgrade, Path},
    http::StatusCode,
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

#[instrument(skip(ctx, token, ws))]
async fn ws_handler(
    Extension(ctx): Extension<Arc<Context>>,
    Path(workspace): Path<String>,
    Query(Param { token }): Query<Param>,
    ws: WebSocketUpgrade,
) -> Response {
    let key = ctx.key.jwt_decode.clone();
    let user = decode::<Claims>(&token, &key, &Validation::default())
        .map(|d| d.claims)
        .ok()
        .map(|claims| claims.user.id);

    let Some(user_id) = user else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    if !ctx
        .db
        .can_read_workspace(user_id.clone(), workspace.clone())
        .await
        .ok()
        .unwrap_or(false)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    ws.protocols(["AFFiNE"]).on_upgrade(move |socket| {
        handle_connector(ctx.clone(), workspace.clone(), user_id, move || {
            socket_connector(socket, &workspace)
        })
    })
}
