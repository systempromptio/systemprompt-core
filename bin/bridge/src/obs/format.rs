//! Tee log writer duplicating tracing output to console and file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::io::{self, Write};

use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::NonBlocking;
use tracing_subscriber::field::Visit;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FormatEvent, FormatFields, MakeWriter};
use tracing_subscriber::registry::LookupSpan;

use super::tracing_init::FILE_WRITER;

pub(super) struct TeeWriter;

impl<'a> MakeWriter<'a> for TeeWriter {
    type Writer = TeeWriterImpl;
    fn make_writer(&'a self) -> Self::Writer {
        TeeWriterImpl {
            file: FILE_WRITER.get().cloned(),
        }
    }
}

pub(super) struct TeeWriterImpl {
    file: Option<NonBlocking>,
}

impl Write for TeeWriterImpl {
    // Falls back to stderr so bootstrap errors before the file writer stay visible.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(file) = self.file.as_mut() {
            _ = file.write_all(buf);
        } else {
            _ = io::stderr().write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            _ = file.flush();
        } else {
            _ = io::stderr().flush();
        }
        Ok(())
    }
}

pub(super) struct BridgeFormat;

#[derive(Default)]
struct EventVisitor {
    message: String,
    fields: String,
}

impl EventVisitor {
    fn write_field(&mut self, name: &str, value: fmt::Arguments<'_>) {
        use std::fmt::Write as _;
        if name == "message" {
            _ = write!(self.message, "{value}");
        } else {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            _ = write!(self.fields, "{name}={value}");
        }
    }
}

impl Visit for EventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.write_field(field.name(), format_args!("{value}"));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.write_field(field.name(), format_args!("{value:?}"));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.write_field(field.name(), format_args!("{value}"));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.write_field(field.name(), format_args!("{value}"));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.write_field(field.name(), format_args!("{value}"));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.write_field(field.name(), format_args!("{value}"));
    }
}

impl<S, N> FormatEvent<S, N> for BridgeFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        let level = event.metadata().level();
        let unquoted = strip_debug_quotes(&visitor.message);
        let tag = crate::brand::brand().binary_name;
        if visitor.fields.is_empty() {
            writeln!(writer, "[{tag}] {level} {unquoted}")
        } else {
            writeln!(writer, "[{tag}] {level} {unquoted} {}", visitor.fields)
        }
    }
}

fn strip_debug_quotes(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}
