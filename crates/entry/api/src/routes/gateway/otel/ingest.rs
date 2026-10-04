//! Persistence of decoded OTLP spans and log records; metrics are summarised
//! only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;
use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, enqueue_background};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

use super::convert::{attrs_to_json, hex_lower, severity_to_level};

const MODULE: &str = "otel";
// Why: OTLP `Status.code` — 0 UNSET, 1 OK, 2 ERROR
// (opentelemetry/proto/trace/v1/trace.proto).
const OTLP_STATUS_CODE_ERROR: i32 = 2;

pub fn ingest_traces(req: ExportTraceServiceRequest) {
    for resource in req.resource_spans {
        let resource_attrs = attrs_to_json(
            resource
                .resource
                .as_ref()
                .map_or(&[][..], |r| r.attributes.as_slice()),
        );
        for scope in resource.scope_spans {
            for span in scope.spans {
                let trace_hex = hex_lower(&span.trace_id);
                let span_hex = hex_lower(&span.span_id);
                let parent_hex = hex_lower(&span.parent_span_id);
                let metadata = json!({
                    "kind": "span",
                    "trace_id": trace_hex,
                    "span_id": span_hex,
                    "parent_span_id": parent_hex,
                    "scope": "claude-desktop",
                    "start_time_unix_nano": span.start_time_unix_nano,
                    "end_time_unix_nano": span.end_time_unix_nano,
                    "duration_ns": span
                        .end_time_unix_nano
                        .saturating_sub(span.start_time_unix_nano),
                    "status_code": span.status.as_ref().map(|s| s.code),

                    "attributes": record_attributes(&span.attributes),
                    "resource": resource_attrs.clone(),
                });
                let level = span
                    .status
                    .as_ref()
                    .filter(|s| s.code == OTLP_STATUS_CODE_ERROR)
                    .map_or(LogLevel::Info, |_| LogLevel::Error);

                let trace_id = if trace_hex.is_empty() {
                    TraceId::system()
                } else {
                    TraceId::new(trace_hex)
                };
                let actor = match actor(&resource_attrs, trace_id) {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::warn!(error = %e, "otel: span log skipped, system admin not initialized");
                        continue;
                    },
                };
                let entry =
                    LogEntry::new(level, MODULE, "Desktop telemetry span".to_owned(), actor)
                        .with_metadata(metadata);
                enqueue_background(entry);
            }
        }
    }
}

pub fn ingest_logs(req: ExportLogsServiceRequest) {
    for resource in req.resource_logs {
        let resource_attrs = attrs_to_json(
            resource
                .resource
                .as_ref()
                .map_or(&[][..], |r| r.attributes.as_slice()),
        );
        for scope in resource.scope_logs {
            for record in scope.log_records {
                let trace_hex = hex_lower(&record.trace_id);
                let span_hex = hex_lower(&record.span_id);
                let metadata = json!({
                    "kind": "log",
                    "trace_id": trace_hex,
                    "span_id": span_hex,
                    "scope": "claude-desktop",
                    "severity_number": record.severity_number,
                    "severity_text": "redacted",
                    "time_unix_nano": record.time_unix_nano,
                    "observed_time_unix_nano": record.observed_time_unix_nano,
                    "attributes": record_attributes(&record.attributes),
                    "resource": resource_attrs.clone(),
                });
                let level = severity_to_level(record.severity_number);
                let message = "Desktop telemetry log".to_owned();
                let trace_id = if trace_hex.is_empty() {
                    TraceId::system()
                } else {
                    TraceId::new(trace_hex)
                };
                let actor = match actor(&resource_attrs, trace_id) {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::warn!(error = %e, "otel: log skipped, system admin not initialized");
                        continue;
                    },
                };
                let entry = LogEntry::new(level, MODULE, message, actor).with_metadata(metadata);
                enqueue_background(entry);
            }
        }
    }
}

pub fn ingest_metrics(req: &ExportMetricsServiceRequest) {
    let total: usize = req
        .resource_metrics
        .iter()
        .flat_map(|resource| &resource.scope_metrics)
        .map(|scope| scope.metrics.len())
        .sum();
    tracing::debug!(total, "otel: metrics export");
}

// JSON: Sanitized OTLP resource attributes carry server-bound identity metadata.
fn actor(
    resource: &serde_json::Value,
    trace: TraceId,
) -> Result<LogActor, systemprompt_logging::LogAttributionUnset> {
    if let (Some(user), Some(session)) = (
        resource["systemprompt.user.id"].as_str(),
        resource["systemprompt.session.id"].as_str(),
    ) {
        return Ok(LogActor::new(
            UserId::new(user.to_owned()),
            SessionId::new(session.to_owned()),
            trace,
        ));
    }
    LogActor::platform(trace)
}

// JSON: Allowlisted OTLP attributes form the persisted log metadata envelope.
fn record_attributes(
    attributes: &[opentelemetry_proto::tonic::common::v1::KeyValue],
) -> serde_json::Value {
    let mut metadata = attrs_to_json(attributes);
    if let Some(object) = metadata.as_object_mut() {
        object.remove("systemprompt.user.id");
        object.remove("systemprompt.session.id");
    }
    metadata
}
