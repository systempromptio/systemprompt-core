//! `logs` rows → OTLP log records. Pure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::AnyValue;
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs, SeverityNumber};

use super::attrs::{Attrs, resource, scope};
use super::ids::{span_id_bytes, trace_id_bytes, unix_nanos};
use super::spans::REQUEST_SPAN;
use super::tail::LogRow;
use systemprompt_identifiers::{ClientId, ContextId, SessionId, TraceId, UserId};

// Why: OTLP `SeverityNumber` buckets (opentelemetry/proto/logs/v1/logs.proto)
// mapped from the five levels `log.sql` permits.
#[must_use]
pub fn severity_number(level: &str) -> SeverityNumber {
    match level {
        "TRACE" => SeverityNumber::Trace,
        "DEBUG" => SeverityNumber::Debug,
        "INFO" => SeverityNumber::Info,
        "WARN" => SeverityNumber::Warn,
        "ERROR" => SeverityNumber::Error,
        _ => SeverityNumber::Unspecified,
    }
}

#[must_use]
pub(super) fn to_export_request(
    rows: &[LogRow],
    instance_id: Option<&str>,
) -> ExportLogsServiceRequest {
    ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs {
            resource: Some(resource(instance_id)),
            scope_logs: vec![ScopeLogs {
                scope: Some(scope()),
                log_records: rows.iter().map(to_log_record).collect(),
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    }
}

#[must_use]
pub fn to_log_record(row: &LogRow) -> LogRecord {
    let mut attrs = Attrs::new();
    attrs
        .text("systemprompt.log.id", &row.id)
        .text("code.namespace", &row.module)
        .opt_str("enduser.id", row.user_id.as_ref().map(UserId::as_str))
        .opt_str("session.id", row.session_id.as_ref().map(SessionId::as_str))
        .opt_str(
            "systemprompt.trace.id",
            row.trace_id.as_ref().map(TraceId::as_str),
        )
        .opt_str(
            "systemprompt.context.id",
            row.context_id.as_ref().map(ContextId::as_str),
        )
        .opt_str(
            "systemprompt.client.id",
            row.client_id.as_ref().map(ClientId::as_str),
        )
        .opt_str("systemprompt.instance.id", row.instance_id.as_deref())
        .opt_str(
            "systemprompt.provider_request.id",
            row.provider_request_id.as_deref(),
        )
        .opt_str(
            "systemprompt.gateway_conversation.id",
            row.gateway_conversation_id.as_deref(),
        )
        .opt_str("systemprompt.log.metadata", row.metadata.as_deref());
    let at = unix_nanos(row.timestamp);
    // Why: a log written under a gateway trace correlates to the request
    // span the traces signal emits for it — same digest, same ids.
    let trace_id = row
        .trace_id
        .as_ref()
        .map(TraceId::as_str)
        .filter(|t| !t.is_empty())
        .map(trace_id_bytes)
        .unwrap_or_default();
    let span_id = row
        .provider_request_id
        .as_deref()
        .filter(|_| !trace_id.is_empty())
        .map(|id| span_id_bytes(REQUEST_SPAN, id))
        .unwrap_or_default();
    LogRecord {
        time_unix_nano: at,
        observed_time_unix_nano: at,
        severity_number: severity_number(&row.level) as i32,
        severity_text: row.level.clone(),
        body: Some(AnyValue {
            value: Some(Value::StringValue(row.message.clone())),
        }),
        attributes: attrs.finish(),
        trace_id,
        span_id,
        ..Default::default()
    }
}
