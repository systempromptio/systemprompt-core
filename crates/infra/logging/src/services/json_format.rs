//! Console event formatters: JSON lines, and the switch between text and JSON.
//!
//! [`JsonEventFormat`] renders one JSON object per event — `timestamp`,
//! `level`, `target`, `fields`, `spans` and `instance` — and routes every field
//! through the same [`is_redacted`] rule as the text formatter, so switching
//! `observability.log_format` never widens what reaches stderr. Span fields are
//! the text the span's own field formatter already redacted.
//!
//! [`ConsoleEventFormat`] is the formatter the global subscriber installs. It
//! reads the process-wide choice made by [`set_console_format`], because the
//! subscriber is installed before the profile that names the format is loaded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Map, Value};
use systemprompt_manifest::profile::LogFormat;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::{Format, Full, Writer};
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

use crate::layer::InstanceLabel;
use crate::sanitize::{REDACTION_PLACEHOLDER, is_redacted, is_system_sentinel};

static JSON_CONSOLE: AtomicBool = AtomicBool::new(false);

pub fn set_console_format(format: LogFormat) {
    JSON_CONSOLE.store(matches!(format, LogFormat::Json), Ordering::Relaxed);
}

#[must_use]
pub fn console_format() -> LogFormat {
    if JSON_CONSOLE.load(Ordering::Relaxed) {
        LogFormat::Json
    } else {
        LogFormat::Text
    }
}

/// One JSON object per event, with sensitive field values redacted.
#[derive(Debug, Clone, Default)]
pub struct JsonEventFormat {
    instance: InstanceLabel,
}

/// The installed console formatter: text or JSON per [`set_console_format`].
#[derive(Debug, Clone, Default)]
pub struct ConsoleEventFormat {
    text: Format<Full, SystemTime>,
    json: JsonEventFormat,
}

impl ConsoleEventFormat {
    pub(crate) fn labelled(instance: InstanceLabel) -> Self {
        Self {
            text: Format::default(),
            json: JsonEventFormat { instance },
        }
    }
}

impl<S, N> FormatEvent<S, N> for ConsoleEventFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        if JSON_CONSOLE.load(Ordering::Relaxed) {
            self.json.format_event(ctx, writer, event)
        } else {
            self.text.format_event(ctx, writer, event)
        }
    }
}

impl<S, N> FormatEvent<S, N> for JsonEventFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();
        let mut fields = JsonFieldVisitor::default();
        event.record(&mut fields);

        let mut object = Map::new();
        object.insert(
            "timestamp".to_owned(),
            Value::String(chrono::Utc::now().to_rfc3339()),
        );
        object.insert("level".to_owned(), Value::String(meta.level().to_string()));
        object.insert("target".to_owned(), Value::String(meta.target().to_owned()));
        object.insert("fields".to_owned(), Value::Object(fields.fields));

        let spans: Vec<Value> = ctx
            .event_scope()
            .into_iter()
            .flat_map(tracing_subscriber::registry::Scope::from_root)
            .map(|span| {
                let mut entry = Map::new();
                entry.insert("name".to_owned(), Value::String(span.name().to_owned()));
                if let Some(rendered) = span.extensions().get::<FormattedFields<N>>()
                    && !rendered.is_empty()
                {
                    entry.insert("fields".to_owned(), Value::String(rendered.to_string()));
                }
                Value::Object(entry)
            })
            .collect();
        if !spans.is_empty() {
            object.insert("spans".to_owned(), Value::Array(spans));
        }
        if let Some(instance_id) = self.instance.get() {
            object.insert(
                "instance".to_owned(),
                Value::String(instance_id.as_str().to_owned()),
            );
        }

        let line = serde_json::to_string(&Value::Object(object)).map_err(|_encode| fmt::Error)?;
        writeln!(writer, "{line}")
    }
}

#[derive(Debug, Default)]
struct JsonFieldVisitor {
    // JSON: a log event's fields are open-ended by definition; this is the
    // outgoing line being built, never parsed.
    fields: Map<String, Value>,
}

impl JsonFieldVisitor {
    // JSON: see `fields`.
    fn insert(&mut self, field: &Field, value: Value) {
        let value = if is_redacted(field.name()) {
            Value::String(REDACTION_PLACEHOLDER.to_owned())
        } else {
            value
        };
        self.fields.insert(field.name().to_owned(), value);
    }
}

impl Visit for JsonFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let rendered = format!("{value:?}");
        if is_system_sentinel(&rendered) {
            return;
        }
        self.insert(field, Value::String(rendered));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if is_system_sentinel(value) {
            return;
        }
        self.insert(field, Value::String(value.to_owned()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.insert(field, Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, Value::from(value));
    }
}
