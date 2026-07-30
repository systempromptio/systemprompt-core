//! Tee log writer duplicating tracing output to console and file.
//!
//! WARN and above reach stderr as well as the rolling log; INFO and below are
//! file-only. Every bridge subcommand reports failures through `tracing`, so
//! without the stderr leg a non-zero exit tells the operator nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::io::{self, Write};

use tracing::{Event, Level, Metadata, Subscriber};
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
            stderr: true,
        }
    }

    // A subcommand exits without ever touching the terminal unless its
    // diagnostics also reach stderr: the whole CLI reports through `tracing`,
    // and once the rolling appender installs, the file is the only sink. WARN
    // and above is the operator's channel; INFO and below stay in the log so
    // `run`'s per-request proxy chatter does not flood a console or journal.
    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        TeeWriterImpl {
            file: FILE_WRITER.get().cloned(),
            stderr: meta.level() <= &Level::WARN,
        }
    }
}

pub(super) struct TeeWriterImpl {
    file: Option<NonBlocking>,
    stderr: bool,
}

impl Write for TeeWriterImpl {
    // Writes stderr when the file writer is absent too, so bootstrap errors
    // raised before the appender installs stay visible.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.stderr || self.file.is_none() {
            _ = io::stderr().write_all(buf);
        }
        if let Some(file) = self.file.as_mut() {
            _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.stderr || self.file.is_none() {
            _ = io::stderr().flush();
        }
        if let Some(file) = self.file.as_mut() {
            _ = file.flush();
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
