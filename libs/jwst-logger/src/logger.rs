use super::*;
use std::io::{stderr, stdout};
use std::sync::Once;
use tracing::Level;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static INIT: Once = Once::new();

/// Initialize a logger with the value of the given environment variable.
///
/// See [`EnvFilter::from_env`]
#[inline]
pub fn init_logger(name: &str) {
    let name = name.replace('-', "_");
    let env_filter = EnvFilter::try_from_env(name.to_uppercase() + "_LOG")
        .unwrap_or_else(|_| EnvFilter::new(name + "=info"));
    init_logger_with_env_filter(env_filter);
}

/// Initialize a logger with the directives in the given string.
///
/// See [`EnvFilter::new`]
#[inline]
pub fn init_logger_with(directives: &str) {
    let env_filter = EnvFilter::new(directives.replace('-', "_"));
    init_logger_with_env_filter(env_filter);
}

fn init_logger_with_env_filter(env_filter: EnvFilter) {
    let writer = stderr.with_max_level(Level::ERROR).or_else(stdout);

    INIT.call_once(|| {
        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .map_writer(move |_| writer)
                    .map_event_format(|_| JWSTFormatter),
            )
            .with(env_filter)
            .init()
    });
}

#[test]
fn test_init_logger() {
    // just test that can be called without panicking
    init_logger("jwst");
}
