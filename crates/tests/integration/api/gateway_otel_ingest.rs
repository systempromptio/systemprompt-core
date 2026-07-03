//! Integration tests (coverage campaign 2026-07).
//!
//! The unauthenticated `/otel` ingest handler always answers `202 Accepted`,
//! swallowing empty bodies, oversize payloads, and bytes that decode as no
//! known OTLP envelope. Protobuf-envelope decode paths (traces/logs/metrics)
//! need `opentelemetry-proto` and live in the unit crate.

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use systemprompt_api::routes::gateway::otel::handle;

fn post(body: Body) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/otel")
        .body(body)
        .expect("request build")
}

#[tokio::test]
async fn empty_body_is_accepted() {
    let resp = handle(post(Body::empty())).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn undecodable_bytes_are_accepted() {
    let resp = handle(post(Body::from(vec![0x01, 0x02, 0x03, 0x04, 0xff, 0xfe]))).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn oversize_body_is_accepted() {
    let oversize = vec![0u8; 4 * 1024 * 1024 + 1024];
    let resp = handle(post(Body::from(oversize))).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}
