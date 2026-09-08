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
    // Why: a closed stderr (a detached GUI, a parent that went away) must
    // not stop the line reaching the log file, and vice versa; both legs run
    // and the first failure is reported after.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stderr = if self.stderr || self.file.is_none() {
            io::stderr().write_all(buf)
        } else {
            Ok(())
        };
        let file = self.file.as_mut().map_or(Ok(()), |f| f.write_all(buf));
        stderr.and(file).map(|()| buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let stderr = if self.stderr || self.file.is_none() {
            io::stderr().flush()
        } else {
            Ok(())
        };
        let file = self.file.as_mut().map_or(Ok(()), Write::flush);
        stderr.and(file)
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
        if name == "message" {
            self.message.push_str(&value.to_string());
        } else {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            self.fields.push_str(&format!("{name}={value}"));
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
