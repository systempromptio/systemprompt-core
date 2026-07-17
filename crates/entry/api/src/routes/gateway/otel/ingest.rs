//! Persistence of decoded OTLP spans and log records; metrics are summarised
//! only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;
use systemprompt_identifiers::TraceId;
use systemprompt_logging::{LogActor, LogEntry, LogLevel, enqueue_background};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

use super::convert::{any_value_to_string, attrs_to_json, hex_lower, severity_to_level};

const MODULE: &str = "otel";

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn ingest_traces(req: ExportTraceServiceRequest) {
    for resource in req.resource_spans {
        let resource_attrs = attrs_to_json(
            resource
                .resource
                .as_ref()
                .map_or(&[][..], |r| r.attributes.as_slice()),
        );
        for scope in resource.scope_spans {
            let scope_name = scope
                .scope
                .as_ref()
                .map_or_else(String::new, |s| s.name.clone());
            for span in scope.spans {
                let trace_hex = hex_lower(&span.trace_id);
                let span_hex = hex_lower(&span.span_id);
                let parent_hex = hex_lower(&span.parent_span_id);
                let metadata = json!({
                    "kind": "span",
                    "trace_id": trace_hex,
                    "span_id": span_hex,
                    "parent_span_id": parent_hex,
                    "scope": scope_name,
                    "start_time_unix_nano": span.start_time_unix_nano,
                    "end_time_unix_nano": span.end_time_unix_nano,
                    "duration_ns": span
                        .end_time_unix_nano
                        .saturating_sub(span.start_time_unix_nano),
                    "status_code": span.status.as_ref().map(|s| s.code),
                    "status_message": span.status.as_ref().map(|s| s.message.clone()),
                    "attributes": attrs_to_json(&span.attributes),
                    "resource": resource_attrs.clone(),
                });
                let level = span
                    .status
                    .as_ref()
                    .filter(|s| s.code == 2) // STATUS_CODE_ERROR
                    .map_or(LogLevel::Info, |_| LogLevel::Error);

                let trace_id = if trace_hex.is_empty() {
                    TraceId::system()
                } else {
                    TraceId::new(trace_hex)
                };
                let actor = match LogActor::platform(trace_id) {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::warn!(error = %e, "otel: span log skipped, system admin not initialized");
                        continue;
                    },
                };
                let entry = LogEntry::new(
                    level,
                    MODULE,
                    if span.name.is_empty() {
                        "<unnamed-span>".to_owned()
                    } else {
                        span.name.clone()
                    },
                    actor,
                )
                .with_metadata(metadata);
                enqueue_background(entry);
            }
        }
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn ingest_logs(req: ExportLogsServiceRequest) {
    for resource in req.resource_logs {
        let resource_attrs = attrs_to_json(
            resource
                .resource
                .as_ref()
                .map_or(&[][..], |r| r.attributes.as_slice()),
        );
        for scope in resource.scope_logs {
            let scope_name = scope
                .scope
                .as_ref()
                .map_or_else(String::new, |s| s.name.clone());
            for record in scope.log_records {
                let trace_hex = hex_lower(&record.trace_id);
                let span_hex = hex_lower(&record.span_id);
                let body_text = any_value_to_string(record.body.as_ref());
                let metadata = json!({
                    "kind": "log",
                    "trace_id": trace_hex,
                    "span_id": span_hex,
                    "scope": scope_name,
                    "severity_number": record.severity_number,
                    "severity_text": record.severity_text,
                    "time_unix_nano": record.time_unix_nano,
                    "observed_time_unix_nano": record.observed_time_unix_nano,
                    "attributes": attrs_to_json(&record.attributes),
                    "resource": resource_attrs.clone(),
                });
                let level = severity_to_level(record.severity_number);
                let message = if body_text.is_empty() {
                    if record.severity_text.is_empty() {
                        "<otel-log>".to_owned()
                    } else {
                        record.severity_text.clone()
                    }
                } else {
                    body_text
                };
                let trace_id = if trace_hex.is_empty() {
                    TraceId::system()
                } else {
                    TraceId::new(trace_hex)
                };
                let actor = match LogActor::platform(trace_id) {
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

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn ingest_metrics(req: &ExportMetricsServiceRequest) {
    let mut total = 0usize;
    let mut names: Vec<String> = Vec::new();
    for resource in &req.resource_metrics {
        for scope in &resource.scope_metrics {
            for m in &scope.metrics {
                total += 1;
                if names.len() < 16 {
                    names.push(m.name.clone());
                }
            }
        }
    }
    tracing::debug!(total, names = ?names, "otel: metrics export");
}
