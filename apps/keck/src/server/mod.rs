mod collaboration;
mod files;

use crate::utils::*;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;

pub async fn start_server() {
    let app = Router::new()
        .nest(
            "/collaboration/:workspace",
            post(collaboration::auth_handler).get(collaboration::upgrade_handler),
        )
        .fallback(get(files::handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
