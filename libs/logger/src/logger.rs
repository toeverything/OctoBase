use super::*;
use fern::{
    colors::{Color, ColoredLevelConfig},
    Dispatch,
};

#[inline]
pub fn init_logger(level: Level) -> Result<(), log::SetLoggerError> {
    let colors = ColoredLevelConfig::new()
        .trace(Color::Black)
        .debug(Color::White)
        .info(Color::Green)
        .warn(Color::Yellow)
        .debug(Color::Red);
    Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}][{:>5}][{}] {}",
                chrono::Local::now().format("[%m-%d %H:%M:%S]"),
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
