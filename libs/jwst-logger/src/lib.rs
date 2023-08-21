#[forbid(unsafe_code)]
mod formatter;
mod logger;

use formatter::JWSTFormatter;
pub use logger::{init_logger, init_logger_with};
pub use tracing::{
    self, debug, debug_span, error, error_span, info, info_span, instrument, log::LevelFilter, trace, trace_span, warn,
    warn_span, Level,
};
