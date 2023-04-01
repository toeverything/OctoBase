mod filter;
mod formatter;
mod logger;

pub use logger::{init_logger, init_logger_with_level, internal_init_logger_with_level};
pub use tracing::{
    self, debug, debug_span, error, error_span, info, info_span, instrument, log::LevelFilter,
    trace, trace_span, warn, warn_span, Level,
};

use filter::GeneralFilter;
use formatter::JWSTFormatter;
