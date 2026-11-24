//! Pipeline items → OTLP records: a persisted [`LogEntry`] becomes a log
//! record, a closed span a span. Pure: no I/O.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use opentelemetry_proto::tonic::common::v1::AnyValue;
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::logs::v1::{LogRecord, SeverityNumber};
use opentelemetry_proto::tonic::trace::v1::Span;
use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use systemprompt_identifiers::{ClientId, ContextId, InstanceId, TaskId, TraceId};

use super::attrs::Attrs;
use super::ids::{trace_id_bytes, unix_nanos};
use crate::models::{LogEntry, LogLevel};

pub const SPAN_KIND: &str = "systemprompt.span.kind";

#[must_use]
pub const fn severity_number(level: LogLevel) -> SeverityNumber {
    match level {
        LogLevel::Trace => SeverityNumber::Trace,
        LogLevel::Debug => SeverityNumber::Debug,
        LogLevel::Info => SeverityNumber::Info,
        LogLevel::Warn => SeverityNumber::Warn,
        LogLevel::Error => SeverityNumber::Error,
    }
}

#[must_use]
pub fn to_log_record(entry: &LogEntry) -> LogRecord {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.log.id", entry.id.as_str())
        .text("code.namespace", &entry.module)
        .text("enduser.id", entry.user_id.as_str())
        .text("session.id", entry.session_id.as_str())
        .text("systemprompt.trace.id", entry.trace_id.as_str())
        .opt_str(
            "systemprompt.task.id",
            entry.task_id.as_ref().map(TaskId::as_str),
        )
        .opt_str(
            "systemprompt.context.id",
            entry.context_id.as_ref().map(ContextId::as_str),
        )
        .opt_str(
            "systemprompt.client.id",
            entry.client_id.as_ref().map(ClientId::as_str),
        )
        .opt_str(
            "systemprompt.instance.id",
            entry.instance_id.as_ref().map(InstanceId::as_str),
        );
    if let Some(metadata) = &entry.metadata {
        attrs.text("systemprompt.log.metadata", &metadata.to_string());
    }
    let at = unix_nanos(entry.timestamp);
    LogRecord {
        time_unix_nano: at,
        observed_time_unix_nano: at,
        severity_number: severity_number(entry.level) as i32,
        severity_text: entry.level.as_str().to_owned(),
        body: Some(AnyValue {
            value: Some(Value::StringValue(entry.message.clone())),
        }),
        attributes: attrs.finish(),
        trace_id: trace_bytes(&entry.trace_id),
        ..Default::default()
    }
}

fn trace_bytes(trace: &TraceId) -> Vec<u8> {
    let key = trace.as_str();
    if key.is_empty() {
        Vec::new()
    } else {
        trace_id_bytes(key)
    }
}

/// A span the pipeline saw close, with the attribution it carried.
#[derive(Debug, Clone)]
pub(crate) struct ClosedSpan {
    pub name: &'static str,
    pub target: &'static str,
    pub trace: TraceId,
    pub span_id: Vec<u8>,
    pub parent_span_id: Vec<u8>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub user: Option<String>,
    pub session: Option<String>,
    pub task: Option<String>,
    pub context: Option<String>,
    pub client: Option<String>,
}

impl ClosedSpan {
    #[must_use]
    pub(crate) fn into_span(self) -> Span {
        let mut attrs = Attrs::new();
        attrs
            .text("code.namespace", self.target)
            .text("systemprompt.trace.id", self.trace.as_str())
            .opt_str("enduser.id", self.user.as_deref())
            .opt_str("session.id", self.session.as_deref())
            .opt_str("systemprompt.task.id", self.task.as_deref())
            .opt_str("systemprompt.context.id", self.context.as_deref())
            .opt_str("systemprompt.client.id", self.client.as_deref())
            .text(SPAN_KIND, self.name);
        Span {
            trace_id: trace_bytes(&self.trace),
            span_id: self.span_id,
            parent_span_id: self.parent_span_id,
            name: self.name.to_owned(),
            kind: SpanKind::Internal as i32,
            start_time_unix_nano: unix_nanos(self.start),
            end_time_unix_nano: unix_nanos(self.end),
            attributes: attrs.finish(),
            ..Default::default()
        }
    }
}
