//! Tests for the OTLP ingest entry point.
//!
//! `handle` is a best-effort sink: an exporter that gets anything other than a
//! 2xx will retry forever and eventually wedge, so every input — empty body,
//! garbage bytes, oversized payload, valid traces or logs — must be accepted.
//! The only observable difference is whether the payload is ingested.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use prost::Message;
use systemprompt_api::routes::gateway::otel::handle;

fn request(body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .body(Body::from(body))
        .unwrap()
}

fn trace_payload() -> Vec<u8> {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: None,
            scope_spans: vec![ScopeSpans {
                scope: None,
                spans: vec![Span {
                    name: "unit-span".to_owned(),
                    ..Default::default()
                }],
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    }
    .encode_to_vec()
}

fn log_payload() -> Vec<u8> {
    ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs {
            resource: None,
            scope_logs: vec![ScopeLogs {
                scope: None,
                log_records: vec![LogRecord::default()],
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    }
    .encode_to_vec()
}

async fn status_of(body: Vec<u8>) -> StatusCode {
    handle(request(body)).await.status()
}

#[tokio::test]
async fn empty_body_is_accepted() {
    assert!(status_of(Vec::new()).await.is_success());
}

#[tokio::test]
async fn undecodable_bytes_are_accepted_rather_than_rejected() {
    // A 4xx here would make the exporter retry the same bad batch forever.
    let status = status_of(vec![0xff; 64]).await;
    assert!(
        status.is_success(),
        "garbage must still be accepted, got {status}"
    );
}

#[tokio::test]
async fn a_trace_export_is_accepted() {
    assert!(status_of(trace_payload()).await.is_success());
}

#[tokio::test]
async fn a_log_export_is_accepted() {
    assert!(status_of(log_payload()).await.is_success());
}

#[tokio::test]
async fn an_empty_resource_spans_list_is_accepted() {
    let empty = ExportTraceServiceRequest {
        resource_spans: vec![],
    }
    .encode_to_vec();
    assert!(status_of(empty).await.is_success());
}

#[tokio::test]
async fn every_input_shape_yields_the_same_status() {
    let mut statuses = vec![
        status_of(Vec::new()).await,
        status_of(vec![0xff; 32]).await,
        status_of(trace_payload()).await,
        status_of(log_payload()).await,
    ];
    statuses.dedup();
    assert_eq!(
        statuses.len(),
        1,
        "the sink must not let callers distinguish outcomes: {statuses:?}"
    );
}
