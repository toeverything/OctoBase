use nu_ansi_term::{AnsiGenericString, Color};
use std::fmt::Result;
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_log::NormalizeEvent;
use tracing_subscriber::{
    fmt::{
        format::{Format, Full, Writer},
        time::FormatTime,
        FmtContext, FormatEvent, FormatFields, FormattedFields,
    },
    registry::LookupSpan,
};

pub struct LogTime;

impl LogTime {
    fn get_time() -> String {
        if cfg!(debug_assertions) {
            chrono::Local::now().format("%m-%d %H:%M:%S").to_string()
        } else {
            chrono::Utc::now().to_rfc3339()
        }
    }
}

impl FormatTime for LogTime {
    fn format_time(&self, w: &mut Writer<'_>) -> Result {
        write!(w, "[{}]", Self::get_time())
    }
}

pub struct JWSTFormatter {
    pub(super) default: Format<Full, LogTime>,
}

impl JWSTFormatter {
    fn format_level(level: &Level) -> AnsiGenericString<str> {
        match *level {
            Level::ERROR => Color::Red.paint("ERROR"),
            Level::WARN => Color::Yellow.paint(" WARN"),
            Level::INFO => Color::Green.paint(" INFO"),
            Level::DEBUG => Color::Blue.paint("DEBUG"),
            Level::TRACE => Color::Purple.paint("TRACE"),
        }
    }

    fn write_log(meta: &Metadata<'_>) -> String {
        if std::env::var("JWST_COLORFUL_LOGS").is_ok() || cfg!(debug_assertions) {
            format!(
                "\r[{}][{}][{}] ",
                Color::DarkGray.paint(LogTime::get_time()),
                Self::format_level(meta.level()),
                Color::LightMagenta.paint(meta.target())
            )
        } else {
            format!(
                "\r[{}][{}][{}] ",
                LogTime::get_time(),
                meta.level().as_str(),
                meta.target()
            )
        }
    }
}

impl<S, N> FormatEvent<S, N> for JWSTFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> Result {
        if cfg!(debug_assertions) {
            let normalized_meta = event.normalized_metadata();
            let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());

            if std::env::var("JWST_DEV").is_err()
                && (meta.target() == "sqlx::query" || meta.target() == "runtime.spawn")
            {
                return Ok(());
            }

            write!(&mut writer, "{}", Self::write_log(meta))?;

            // Format all the spans in the event's span context.
            if let Some(scope) = ctx.event_scope() {
                for span in scope.from_root() {
                    write!(writer, "{}", span.name())?;

                    let ext = span.extensions();
                    let fields = &ext
                        .get::<FormattedFields<N>>()
                        .expect("will never be `None`");

                    // Skip formatting the fields if the span had no fields.
                    if !fields.is_empty() {
                        write!(writer, "{{{fields}}}")?;
                    }
                    write!(writer, ": ")?;
                }
            }

            // Write fields on the event
            ctx.field_format().format_fields(writer.by_ref(), event)?;

            writeln!(writer)
        } else {
            self.default.format_event(ctx, writer, event)
        }
    }
}
