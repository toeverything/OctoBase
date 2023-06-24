use super::{
    blobs,
    blocks::{block, schema, workspace},
    *,
};
use cloud_infra::with_api_doc;
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
        workspace::set_search_index,
        workspace::get_search_index,
        workspace::subscribe_workspace,
        block::get_block,
        block::get_block_by_flavour,
        block::set_block,
        block::get_block_history,
        block::get_block_children,
        block::delete_block,
        block::insert_block_children,
        block::remove_block_children,
        blobs::check_blob,
        blobs::get_blob,
        blobs::set_blob,
        blobs::delete_blob,
    ),
    components(
        schemas(
            blobs::BlobStatus, schema::InsertChildren,
            schema::Workspace, schema::Block, schema::BlockRawHistory, schema::SubscribeWorkspace,
            jwst::BlockHistory, jwst::HistoryOperation, jwst::RawHistory,
            jwst::SearchResults, jwst::SearchResult,
        )
    ),
    tags(
        (name = "Workspace", description = "Read and write remote workspace"),
        (name = "Blocks", description = "Read and write remote blocks")
    )
)]
struct ApiDoc;

const README: &str = include_str!("../../../../homepage/pages/docs/introduction.md");
const DISTINCTIVE_FEATURES: &str =
    include_str!("../../../../homepage/pages/docs/overview/distinctive_features.md");

pub fn doc_apis(router: Router) -> Router {
    if cfg!(feature = "schema") {
        let mut openapi = ApiDoc::openapi();
        openapi.info.description = Some(vec![README, DISTINCTIVE_FEATURES].join("\n"));
        with_api_doc(router, openapi, "jwst")
    } else {
        router
    }
}
