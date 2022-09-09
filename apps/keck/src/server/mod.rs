mod collaboration;
mod files;

use crate::utils::*;
use axum::{
    routing::{get, post},
    Router,
};
use http::{HeaderValue, Method};
use std::net::SocketAddr;
use tower_http::cors::{any, CorsLayer};

pub async fn start_server() {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS])
        // allow requests from any origin
        .allow_origin("http://localhost:4200".parse::<HeaderValue>().unwrap())
        .allow_headers(any());

    let app = Router::new()
        .nest(
            "/collaboration/:workspace",
            post(collaboration::auth_handler).get(collaboration::upgrade_handler),
        )
        .layer(cors)
        .fallback(get(files::handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
