use super::*;
use fern::{
    colors::{Color, ColoredLevelConfig},
    Dispatch,
};
use time::{format_description, OffsetDateTime};

#[inline]
pub fn init_logger(level: Level) -> Result<(), log::SetLoggerError> {
    let colors = ColoredLevelConfig::new()
        .trace(Color::Black)
        .debug(Color::White)
        .info(Color::Green)
        .warn(Color::Yellow)
        .debug(Color::Red);
    let format = format_description::parse("[month]-[day] [hour]:[minute]").unwrap();
    Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}][{:>5}][{}] {}",
                OffsetDateTime::now_utc()
                    .format(&format)
                    .unwrap_or("UnknownTime".to_string()),
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(level.to_level_filter())
        .chain(Box::new(Logger {}) as Box<dyn log::Log>)
        .apply()
}

pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if cfg!(debug_assertions) {
            if option_env!("JWST_DEV").is_some() {
                return metadata.target() != "sqlx::query";
            }
        }
        metadata.level() <= Level::Info && metadata.target() != "sqlx::query"
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{}", record.args());
        }
    }

    fn flush(&self) {}
}
