mod logger;

pub use lazy_static::lazy_static;
pub use log::{debug, error, info, warn, Level, Metadata, Record};
pub use logger::init_logger;
pub use uuid::Uuid;
