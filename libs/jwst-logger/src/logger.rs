use super::*;
use std::io::{stderr, stdout};
use tracing::Level;
use tracing_subscriber::prelude::*;

#[inline]
pub fn init_logger() {
    init_logger_with_level(Level::INFO);
}

#[inline]
pub fn init_logger_with_level(level: Level) {
    internal_init_logger_with_level(if cfg!(debug_assertions) {
        Level::DEBUG
    } else {
        level
    })
}

#[inline]
pub fn internal_init_logger_with_level(level: Level) {
    let writer = stderr
        .with_max_level(Level::WARN)
        .or_else(stdout.with_max_level(level));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .map_writer(move |_| writer)
                .map_event_format(|_| JWSTFormatter)
                .with_filter(GeneralFilter),
        )
        // .with(tracing_stackdriver::layer().with_filter(GeneralFilter))
        .init();
}
