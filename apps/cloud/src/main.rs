use axum::{Extension, Router, Server};
use http::Method;
use jwst_logger::{error, info, init_logger};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

mod api;
mod context;
mod error_status;
mod files;
mod layer;
mod utils;
pub use api::{
    delete_workspace, get_doc, get_public_doc, get_workspace_by_id, get_workspaces, health_check,
    make_token, query_user, search_workspace, update_workspace,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
#[tokio::main]
async fn main() {
    init_logger();
    #[derive(OpenApi)]
    #[openapi(
        paths(
            api::get_workspaces,
            api::get_workspace_by_id,
            api::update_workspace,
            api::delete_workspace,
            api::search_workspace,
            api::query_user,
            api::make_token,
            api::get_doc,
            api::get_public_doc,
            api::health_check,
        ),
        tags(
            (name = "Workspace", description = "Read and write remote workspace"),
        )
    )]
    struct ApiDoc;
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
        ])
        // allow requests from any origin
        .allow_origin(["https://affine-next.vercel.app".parse().unwrap()])
        .allow_headers(Any);

    let context = Arc::new(context::Context::new().await);

    let app = files::static_files(
        Router::new()
            .merge(SwaggerUi::new("/api-docs").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .nest(
                "/api",
                api::make_rest_route(context.clone()).nest("/sync", api::make_ws_route()),
            )
            .layer(Extension(context.clone()))
            .layer(cors),
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
