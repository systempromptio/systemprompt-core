// OTel ingest endpoint.
//
// Trust boundary: this route is unauthenticated by design. Codex starts
// emitting telemetry before any auth handshake completes, and the bridge proxy
// already gates `/otel` to loopback origin (bin/bridge/src/proxy/server.rs). Do
// not add JWT/API-key auth here without coordinating with the bridge.

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use prost::Message;
use serde_json::{Value, json};
use systemprompt_database::DbPool;
use systemprompt_identifiers::TraceId;
use systemprompt_logging::{LogEntry, LogLevel, LoggingRepository};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MODULE: &str = "otel";

pub async fn handle(pool: DbPool, request: Request<Body>) -> Response<Body> {
    let body_bytes = match axum::body::to_bytes(request.into_body(), MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "otel: body read failed");
            return accepted();
        },
    };

    if body_bytes.is_empty() {
        return accepted();
    }

    let repo = match LoggingRepository::new(&pool) {
        // Why: otel ingest is a high-volume background path; keep it strictly
        // database-backed and out of stderr so it cannot drown the operator's
        // terminal or recurse into tracing's own subscriber.
        Ok(r) => r.with_terminal(false).with_database(true),
        Err(e) => {
            tracing::warn!(error = %e, "otel: logging repo unavailable");
            return accepted();
        },
    };

    if let Ok(req) = ExportTraceServiceRequest::decode(body_bytes.as_ref()) {
        if !req.resource_spans.is_empty() {
            ingest_traces(&repo, req).await;
            return accepted();
        }
    }
    if let Ok(req) = ExportLogsServiceRequest::decode(body_bytes.as_ref()) {
        if !req.resource_logs.is_empty() {
            ingest_logs(&repo, req).await;
            return accepted();
        }
    }
    if let Ok(req) = ExportMetricsServiceRequest::decode(body_bytes.as_ref()) {
        if !req.resource_metrics.is_empty() {
            ingest_metrics(&req);
            return accepted();
        }
    }

    tracing::warn!(
        bytes = body_bytes.len(),
        "otel: payload did not decode as any known OTLP envelope"
    );
    accepted()
}

fn accepted() -> Response<Body> {
    Response::builder()
        .status(StatusCode::ACCEPTED)
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

async fn ingest_traces(repo: &LoggingRepository, req: ExportTraceServiceRequest) {
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
                .map(|s| s.name.clone())
                .unwrap_or_else(String::new);
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

                let mut entry = LogEntry::new(
                    level,
                    MODULE,
                    if span.name.is_empty() {
                        "<unnamed-span>".to_string()
                    } else {
                        span.name.clone()
                    },
                )
                .with_metadata(metadata);
                if !trace_hex.is_empty() {
                    entry = entry.with_trace_id(TraceId::new(trace_hex));
                }
                if let Err(e) = repo.log(entry).await {
                    tracing::warn!(error = %e, "otel: span log persist failed");
                }
            }
        }
    }
}

async fn ingest_logs(repo: &LoggingRepository, req: ExportLogsServiceRequest) {
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
                .map(|s| s.name.clone())
                .unwrap_or_else(String::new);
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
                        "<otel-log>".to_string()
                    } else {
                        record.severity_text.clone()
                    }
                } else {
                    body_text
                };
                let mut entry = LogEntry::new(level, MODULE, message).with_metadata(metadata);
                if !trace_hex.is_empty() {
                    entry = entry.with_trace_id(TraceId::new(trace_hex));
                }
                if let Err(e) = repo.log(entry).await {
                    tracing::warn!(error = %e, "otel: log persist failed");
                }
            }
        }
    }
}

fn ingest_metrics(req: &ExportMetricsServiceRequest) {
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

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

const fn severity_to_level(severity_number: i32) -> LogLevel {
    // OTel severity_number: 1-4=TRACE, 5-8=DEBUG, 9-12=INFO, 13-16=WARN,
    // 17-24=ERROR
    match severity_number {
        ..=4 => LogLevel::Trace,
        5..=8 => LogLevel::Debug,
        9..=12 => LogLevel::Info,
        13..=16 => LogLevel::Warn,
        _ => LogLevel::Error,
    }
}

fn any_value_to_string(value: Option<&opentelemetry_proto::tonic::common::v1::AnyValue>) -> String {
    use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
    let Some(av) = value.and_then(|v| v.value.as_ref()) else {
        return String::new();
    };
    match av {
        AV::StringValue(s) => s.clone(),
        AV::BoolValue(b) => b.to_string(),
        AV::IntValue(i) => i.to_string(),
        AV::DoubleValue(f) => f.to_string(),
        AV::BytesValue(b) => format!("<bytes:{}>", b.len()),
        AV::ArrayValue(_) | AV::KvlistValue(_) => serde_json::to_string(&any_value_to_json(value))
            .unwrap_or_else(|e| format!("<json-serialise-failed: {e}>")),
    }
}

fn attrs_to_json(attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue]) -> Value {
    let mut map = serde_json::Map::new();
    for kv in attrs {
        map.insert(kv.key.clone(), any_value_to_json(kv.value.as_ref()));
    }
    Value::Object(map)
}

fn any_value_to_json(value: Option<&opentelemetry_proto::tonic::common::v1::AnyValue>) -> Value {
    use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
    let Some(av) = value.and_then(|v| v.value.as_ref()) else {
        return Value::Null;
    };
    match av {
        AV::StringValue(s) => Value::String(s.clone()),
        AV::BoolValue(b) => Value::Bool(*b),
        AV::IntValue(i) => Value::from(*i),
        AV::DoubleValue(f) => json!(f),
        AV::BytesValue(b) => Value::String(format!("<bytes:{}>", b.len())),
        AV::ArrayValue(arr) => Value::Array(
            arr.values
                .iter()
                .map(|v| any_value_to_json(Some(v)))
                .collect(),
        ),
        AV::KvlistValue(kvs) => {
            let mut map = serde_json::Map::new();
            for kv in &kvs.values {
                map.insert(kv.key.clone(), any_value_to_json(kv.value.as_ref()));
            }
            Value::Object(map)
        },
    }
}
