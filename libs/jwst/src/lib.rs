mod block;
mod history;
mod space;
mod types;
mod utils;
mod workspace;

pub mod constants;

pub use block::Block;
pub use history::{
    parse_history, parse_history_client, BlockHistory, HistoryOperation, RawHistory,
};
pub use space::Space;
pub use tracing::{debug, error, info, log::LevelFilter, trace, warn};
pub use types::{BlobMetadata, BlobStorage, BucketBlobStorage, DocStorage, JwstError, JwstResult};
pub use utils::{
    sync_encode_update, Base64DecodeError, Base64Engine, STANDARD_ENGINE, URL_SAFE_ENGINE,
};
pub use workspace::BlockObserverConfig;
pub use workspace::{MapSubscription, Workspace, WorkspaceMetadata, WorkspaceTransaction};
#[cfg(feature = "workspace-search")]
pub use workspace::{SearchResult, SearchResults};

const RETRY_NUM: i32 = 200;

#[inline]
pub fn print_versions(pkg_name: &str, pkg_version: &str) {
    use convert_case::{Case, Casing};
    info!("{}-{}", pkg_name.to_case(Case::Pascal), pkg_version);
    info!(
        "Based on OctoBase-{}-{}-{}",
        env!("CARGO_PKG_VERSION"),
        &env!("VERGEN_GIT_COMMIT_TIMESTAMP")[0..10],
        &env!("VERGEN_GIT_SHA")[0..7]
    );
    info!(
        "Built with rust {}-{}-{}",
        env!("VERGEN_RUSTC_SEMVER"),
        env!("VERGEN_RUSTC_COMMIT_DATE"),
        &env!("VERGEN_RUSTC_COMMIT_HASH")[0..7],
    );
}

#[test]
fn test_print_versions() {
    // just for test coverage
    print_versions("jwst", "0.1.0");
}
