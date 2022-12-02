use std::{net::SocketAddr, sync::Arc};

use axum::{Extension, Router, Server};
use tracing::{error, info};

mod api;
mod context;
mod database;
mod layer;
mod model;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let context = Arc::new(context::Context::new().await);

    let app = Router::new()
        .nest("/api", api::make_rest_route())
        .layer(layer::make_cors_layer())
        .layer(Extension(context.clone()));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);

    if let Err(e) = Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(utils::shutdown_signal())
        .await
    {
        error!("Server shutdown due to error: {}", e);
    }

    context.db.close().await;

    info!("Server shutdown complete");
}
