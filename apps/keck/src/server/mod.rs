mod api;
mod collaboration;
mod files;
mod utils;

use crate::{server::api::Context, utils::*};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use http::Method;
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

pub async fn start_server() {
    let origins = [
        "http://localhost:4200".parse().unwrap(),
        "http://127.0.0.1:4200".parse().unwrap(),
        "http://localhost:3000".parse().unwrap(),
        "http://127.0.0.1:3000".parse().unwrap(),
    ];

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
        ])
        // allow requests from any origin
        .allow_origin(origins)
        .allow_headers(Any);

    let context = Arc::new(Context::new().await);

    let app = Router::new()
        .merge(api::api_docs())
        // .nest("/api", api::api_handler())
        .nest("/api", api::api_handler())
        .nest(
            "/collaboration/:workspace",
            post(collaboration::auth_handler).get(collaboration::upgrade_handler),
        )
        .layer(cors)
        .layer(Extension(context))
        .nest("/docs", get(files::docs_handler))
        .fallback(get(files::index_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
