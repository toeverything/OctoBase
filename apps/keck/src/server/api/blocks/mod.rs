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
use jwst_static::with_api_doc_v2;
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
        workspace::workspace_search,
        block::get_block,
        block::get_block_by_flavour,
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
            jwst::BlockHistory, jwst::HistoryOperation, jwst::RawHistory,
            jwst::SearchResults, jwst::SearchResult
        )
    ),
    tags(
        (name = "Workspace", description = "Read and write remote workspace"),
        (name = "Blocks", description = "Read and write remote blocks")
    )
)]
struct ApiDoc;

const README: &str = include_str!("../../../../../homepage/pages/docs/introduction.md");
const DISTINCTIVE_FEATURES: &str =
    include_str!("../../../../../homepage/pages/docs/overview/distinctive_features.md");

fn doc_apis(router: Router) -> Router {
    if cfg!(feature = "schema") {
        let mut openapi = ApiDoc::openapi();
        openapi.info.description = Some(vec![README, DISTINCTIVE_FEATURES].join("\n"));
        with_api_doc_v2(router, openapi, "JWST Api Docs")
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
            "/block/:workspace/flavour/:flavour",
            get(block::get_block_by_flavour),
        )
        .route(
            "/block/:workspace/blocks",
            get(workspace::get_workspace_block),
        )
        .route("/search/:workspace", get(workspace::workspace_search))
        .route(
            "/search/:workspace/index",
            get(workspace::get_search_index).post(workspace::set_search_index),
        )
}

pub fn blocks_apis(router: Router) -> Router {
    workspace_apis(block_apis(router))
}
