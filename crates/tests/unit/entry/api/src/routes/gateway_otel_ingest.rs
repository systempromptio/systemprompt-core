//! OTLP ingest persistence paths (`routes::gateway::otel::ingest`). Builds
//! protobuf envelopes for traces, logs, and metrics and drives the ingest
//! functions directly through the `test_api` seam. Without a bootstrapped
//! system admin the per-record `LogActor::platform` call fails closed and the
//! record is skipped, but the decode/flatten/metadata assembly still runs — the
//! functions must never panic on any well-formed envelope.

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value::Value as AV;
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::metrics::v1::{Metric, ResourceMetrics, ScopeMetrics};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status};
use systemprompt_api::routes::gateway::otel::test_api::{
    ingest_logs, ingest_metrics, ingest_traces,
};

fn string_kv(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.to_owned(),
        key_strindex: 0,
        value: Some(AnyValue {
            value: Some(AV::StringValue(value.to_owned())),
        }),
    }
}

fn resource(attrs: Vec<KeyValue>) -> Resource {
    Resource {
        attributes: attrs,
        dropped_attributes_count: 0,
        entity_refs: Vec::new(),
    }
}

fn scope() -> InstrumentationScope {
    InstrumentationScope {
        name: "test-scope".to_owned(),
        version: "1.0".to_owned(),
        attributes: Vec::new(),
        dropped_attributes_count: 0,
    }
}

#[test]
fn ingest_traces_handles_error_and_unnamed_and_rootless_spans() {
    let error_span = Span {
        trace_id: vec![0xaa; 16],
        span_id: vec![0xbb; 8],
        parent_span_id: vec![0xcc; 8],
        name: "http.request".to_owned(),
        start_time_unix_nano: 100,
        end_time_unix_nano: 250,
        attributes: vec![string_kv("http.method", "GET")],
        status: Some(Status {
            message: "boom".to_owned(),
            code: 2,
        }),
        ..Span::default()
    };
    let unnamed_rootless = Span {
        trace_id: Vec::new(),
        span_id: vec![0x11; 8],
        name: String::new(),
        start_time_unix_nano: 5,
        end_time_unix_nano: 1,
        ..Span::default()
    };
    let req = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(resource(vec![string_kv("service.name", "gw")])),
            scope_spans: vec![ScopeSpans {
                scope: Some(scope()),
                spans: vec![error_span, unnamed_rootless],
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    };
    ingest_traces(req);
}

#[test]
fn ingest_logs_handles_bodied_and_empty_records() {
    let bodied = LogRecord {
        time_unix_nano: 1_000,
        observed_time_unix_nano: 1_100,
        severity_number: 17,
        severity_text: "ERROR".to_owned(),
        body: Some(AnyValue {
            value: Some(AV::StringValue("disk full".to_owned())),
        }),
        attributes: vec![string_kv("component", "storage")],
        trace_id: vec![0x01; 16],
        span_id: vec![0x02; 8],
        ..LogRecord::default()
    };
    let empty_body_with_severity = LogRecord {
        severity_number: 9,
        severity_text: "INFO".to_owned(),
        ..LogRecord::default()
    };
    let fully_empty = LogRecord::default();
    let req = ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs {
            resource: Some(resource(vec![string_kv("service.name", "gw")])),
            scope_logs: vec![ScopeLogs {
                scope: Some(scope()),
                log_records: vec![bodied, empty_body_with_severity, fully_empty],
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    };
    ingest_logs(req);
}

#[test]
fn ingest_metrics_summarises_names() {
    let metrics: Vec<Metric> = (0..20)
        .map(|i| Metric {
            name: format!("metric_{i}"),
            ..Metric::default()
        })
        .collect();
    let req = ExportMetricsServiceRequest {
        resource_metrics: vec![ResourceMetrics {
            resource: Some(resource(Vec::new())),
            scope_metrics: vec![ScopeMetrics {
                scope: Some(scope()),
                metrics,
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    };
    ingest_metrics(&req);
}

#[test]
fn ingest_functions_tolerate_empty_envelopes() {
    ingest_traces(ExportTraceServiceRequest {
        resource_spans: Vec::new(),
    });
    ingest_logs(ExportLogsServiceRequest {
        resource_logs: Vec::new(),
    });
    ingest_metrics(&ExportMetricsServiceRequest {
        resource_metrics: Vec::new(),
    });
}
