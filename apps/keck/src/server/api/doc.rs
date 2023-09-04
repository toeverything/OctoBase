use utoipa::{openapi::License, OpenApi};
#[cfg(feature = "schema")]
use utoipa_swagger_ui::{serve, Config, Url};

use super::{
    blobs,
    blocks::{block, schema, workspace},
    *,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        workspace::get_workspace,
        workspace::set_workspace,
        workspace::delete_workspace,
        workspace::workspace_client,
        // workspace::history_workspace_clients,
        // workspace::history_workspace,
        workspace::get_workspace_block,
        // workspace::workspace_search,
        // workspace::set_search_index,
        // workspace::get_search_index,
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
            schema::Workspace, schema::Block, schema::BlockRawHistory, schema::SubscribeWorkspace
            // jwst::BlockHistory, jwst::HistoryOperation, jwst::RawHistory,
            // jwst::SearchResults, jwst::SearchResult,
        )
    ),
    tags(
        (name = "Workspace", description = "Read and write remote workspace"),
        (name = "Blocks", description = "Read and write remote blocks")
    )
)]
struct ApiDoc;

const README: &str = include_str!("../../../../homepage/pages/docs/introduction.md");
const DISTINCTIVE_FEATURES: &str = include_str!("../../../../homepage/pages/docs/overview/distinctive_features.md");

#[cfg(feature = "schema")]
async fn serve_swagger_ui(
    tail: Option<Path<String>>,
    Extension(state): Extension<Arc<Config<'static>>>,
) -> impl IntoResponse {
    match serve(&tail.map(|p| p.to_string()).unwrap_or("".into()), state) {
        Ok(file) => file
            .map(|file| (StatusCode::OK, [("Content-Type", file.content_type)], file.bytes).into_response())
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response()),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

pub fn doc_apis(router: Router) -> Router {
    #[cfg(feature = "schema")]
    {
        let mut openapi = ApiDoc::openapi();
        openapi.info.description = Some([README, DISTINCTIVE_FEATURES].join("\n"));

        let name = "jwst";
        if cfg!(debug_assertions) || std::env::var("JWST_DEV").is_ok() {
            let config = Url::from(format!("/api/{name}.json"));
            let config = Config::new(vec![config]);
            openapi.info.license = Some(License::new(env!("CARGO_PKG_LICENSE")));
            router
                .route(&format!("/{name}.json"), get(move || async { Json(openapi) }))
                .route("/docs/", get(serve_swagger_ui))
                .route("/docs/*tail", get(serve_swagger_ui))
                .layer(Extension(Arc::new(config)))
        } else {
            router
        }
    }
    #[cfg(not(feature = "schema"))]
    {
        router
    }
}
