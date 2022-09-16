mod logger;

use log::{Metadata, Record};

pub use log::{debug, error, info, warn, Level};
pub use logger::init_logger;
