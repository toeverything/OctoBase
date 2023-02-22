use axum::{Extension, Router, Server};
use jwst_logger::{error, info, init_logger};
use std::{net::SocketAddr, sync::Arc};

mod api;
mod context;
mod error_status;
mod files;
mod layer;
mod utils;

#[tokio::main]
async fn main() {
    init_logger();

    let context = Arc::new(context::Context::new().await);

    let app = files::static_files(
        Router::new()
            .nest(
                "/api",
                api::make_rest_route(context.clone()).nest("/sync", api::make_ws_route()),
            )
            .layer(Extension(context.clone())),
    );

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);

    if let Err(e) = Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(utils::shutdown_signal())
        .await
    {
        error!("Server shutdown due to error: {}", e);
    }

    info!("Server shutdown complete");
}
