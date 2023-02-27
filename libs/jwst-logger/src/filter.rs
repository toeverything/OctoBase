use tracing::{subscriber::Interest, Level, Metadata};
use tracing_subscriber::layer::{Context, Filter};

pub struct GeneralFilter;

const EXCLUDE_PREFIX: [&str; 4] = [
    "hyper::",
    "rustls::",
    "mio::",
    "tantivy::indexer::segment_updater",
];

impl GeneralFilter {
    fn is_enabled(&self, metadata: &Metadata<'_>) -> bool {
        let target = metadata.target();
        let is_kernel = EXCLUDE_PREFIX
            .iter()
            .any(|prefix| target.starts_with(prefix))
            && *metadata.level() > Level::INFO;

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
