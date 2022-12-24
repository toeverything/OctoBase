use tracing::{subscriber::Interest, Level, Metadata};
use tracing_subscriber::layer::{Context, Filter};

pub struct GeneralFilter;

const EXCLUDE_PREFIX: [&str; 3] = ["hyper::", "rustls::", "mio::"];

impl GeneralFilter {
    fn is_enabled(&self, metadata: &Metadata<'_>) -> bool {
        let is_kernel = EXCLUDE_PREFIX
            .iter()
            .any(|prefix| metadata.target().starts_with(prefix))
            && *metadata.level() > Level::INFO;

        if cfg!(debug_assertions) {
            return metadata.target() != "sqlx::query" && !is_kernel;
        }
        *metadata.level() <= Level::INFO && !is_kernel
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
