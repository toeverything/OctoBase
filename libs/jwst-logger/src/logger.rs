use super::*;
use std::io::{stderr, stdout};
use tracing::Level;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize a logger with the value of the given environment variable.
///
/// See [`EnvFilter::from_env`]
#[inline]
pub fn init_logger(name: &str) {
    let writer = stderr.with_max_level(Level::WARN).or_else(stdout);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .map_writer(move |_| writer)
                .map_event_format(|_| JWSTFormatter),
        )
        .with(EnvFilter::from_env(
            name.replace('-', "_").to_uppercase() + "_LOG",
        ))
        .init();
}

/// Initialize a logger with the directives in the given string.
///
/// See [`EnvFilter::new`]
#[inline]
pub fn init_logger_with(directives: &str) {
    let writer = stderr.with_max_level(Level::WARN).or_else(stdout);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .map_writer(move |_| writer)
                .map_event_format(|_| JWSTFormatter),
        )
        .with(EnvFilter::new(directives.replace('-', "_")))
        .init();
}
