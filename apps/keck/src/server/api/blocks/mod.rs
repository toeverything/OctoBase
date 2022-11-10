mod block;
mod schema;
mod workspace;

pub use block::{
    delete_block, get_block, get_block_history, insert_block_children, remove_block_children,
    set_block,
};
pub use workspace::{
    delete_workspace, get_workspace, history_workspace, history_workspace_clients, set_workspace,
    workspace_client,
};

use super::*;
use schema::InsertChildren;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        workspace::get_workspace,
        workspace::set_workspace,
        workspace::delete_workspace,
        workspace::workspace_client,
        workspace::history_workspace_clients,
        workspace::history_workspace,
        workspace::get_workspace_block,
        block::get_block,
        block::set_block,
        block::get_block_history,
        block::get_block_children,
        block::delete_block,
        block::insert_block_children,
        block::remove_block_children,
    ),
    components(
        schemas(
            schema::InsertChildren,
            schema::Workspace, schema::Block, schema::BlockRawHistory,
            jwst::BlockHistory, jwst::HistoryOperation, jwst::RawHistory
        )
    ),
    tags(
        (name = "Workspace", description = "Read and write remote workspace"),
        (name = "Blocks", description = "Read and write remote blocks")
    )
)]
struct ApiDoc;

const README: &str = include_str!("../../../../../handbook/src/README.md");
const CORE_CONCEPT: &str = include_str!("../../../../../handbook/src/core_concept.md");

fn doc_apis(router: Router) -> Router {
    if cfg!(feature = "schema") {
        use utoipa_swagger_ui::{serve, Config, Url};

        async fn serve_swagger_ui(
            Path(tail): Path<String>,
            Extension(state): Extension<Arc<Config<'static>>>,
        ) -> impl IntoResponse {
            match serve(&tail[1..], state) {
                Ok(file) => file
                    .map(|file| {
                        (
                            StatusCode::OK,
                            [("Content-Type", file.content_type)],
                            file.bytes,
                        )
                            .into_response()
                    })
                    .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response()),
                Err(error) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                }
            }
        }

        let mut openapi = ApiDoc::openapi();
        openapi.info.description = Some(vec![README, CORE_CONCEPT].join("\n"));

        router
            .route("/jwst.json", get(move || async { Json(openapi) }))
            .route("/docs/*tail", get(serve_swagger_ui))
            .layer(Extension(Arc::new(Config::new(vec![Url::new(
                "JWST Api Docs",
                "/api/jwst.json",
            )]))))
    } else {
        router
    }
}

fn block_apis(router: Router) -> Router {
    let block_operation = Router::new()
        .route("/history", get(block::get_block_history))
        .route(
            "/children",
            get(block::get_block_children).post(block::insert_block_children),
        )
        .route("/children/:children", delete(block::remove_block_children));

    doc_apis(router)
        .nest("/block/:workspace/:block/", block_operation)
        .route(
            "/block/:workspace/:block",
            get(block::get_block)
                .post(block::set_block)
                .delete(block::delete_block),
        )
}

fn workspace_apis(router: Router) -> Router {
    router
        .route("/block/:workspace/client", get(workspace::workspace_client))
        .route(
            "/block/:workspace/history",
            get(workspace::history_workspace_clients),
        )
        .route(
            "/block/:workspace/history/:client",
            get(workspace::history_workspace),
        )
        .route(
            "/block/:workspace",
            get(workspace::get_workspace)
                .post(workspace::set_workspace)
                .delete(workspace::delete_workspace),
        )
        .route(
            "/block/:workspace/blocks",
            get(workspace::get_workspace_block),
        )
}

pub fn blocks_apis(router: Router) -> Router {
    let api_handler = workspace_apis(block_apis(Router::new()));

    router.nest("/api", api_handler)
}
