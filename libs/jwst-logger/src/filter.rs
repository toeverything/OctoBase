use tracing::{subscriber::Interest, Level, Metadata};
use tracing_subscriber::layer::{Context, Filter};

pub struct GeneralFilter;

const EXCLUDE_PREFIX_DEBUG: [&str; 3] = ["hyper::", "rustls::", "mio::"];
const EXCLUDE_PREFIX_INFO: [&str; 1] = ["tantivy::indexer::segment_updater"];

impl GeneralFilter {
    fn is_enabled(&self, metadata: &Metadata<'_>) -> bool {
        let target = metadata.target();
        let is_kernel = EXCLUDE_PREFIX_DEBUG
            .iter()
            .any(|prefix| target.starts_with(prefix))
            && *metadata.level() > Level::INFO
            || EXCLUDE_PREFIX_INFO
                .iter()
                .any(|prefix| target.starts_with(prefix))
                && *metadata.level() > Level::WARN;

        if cfg!(debug_assertions) {
            return target != "sqlx::query" && !is_kernel;
        }
        *metadata.level() <= Level::INFO && target != "sqlx::query" && !is_kernel
    }
}

impl<S> Filter<S> for GeneralFilter {
    fn enabled(&self, metadata: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        self.is_enabled(metadata)
    }

    fn callsite_enabled(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.is_enabled(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }
}
