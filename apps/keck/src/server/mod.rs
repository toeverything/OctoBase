mod api;
mod collaboration;
mod files;

use crate::{server::api::Context, utils::*};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use http::{HeaderValue, Method};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

pub async fn start_server() {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS])
        // allow requests from any origin
        .allow_origin("http://localhost:4200".parse::<HeaderValue>().unwrap())
        .allow_headers(Any);

    let context = Arc::new(Context::new());

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
        .fallback(get(files::handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
