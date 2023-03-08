mod filter;
mod formatter;
mod logger;

pub use logger::init_logger;
pub use tracing::{
    debug, debug_span, error, error_span, info, info_span, log::LevelFilter, trace, trace_span,
    warn, warn_span,
};

use filter::GeneralFilter;
use formatter::{JWSTFormatter, LogTime};

#[inline]
pub fn print_versions(pkv_version: &str) {
    info!(
        "OctoBase {}-{}-{}",
        pkv_version,
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
