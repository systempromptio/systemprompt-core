//! OTLP telemetry ingest endpoint.
//!
//! [`handle`] decodes a protobuf OTLP envelope (traces, logs, or metrics) and
//! persists spans and log records to the logging repository (see the `ingest`
//! submodule); metrics are only summarised. It always responds `202 Accepted`,
//! swallowing decode and persist failures so a misbehaving emitter cannot
//! stall.
//!
//! Trust boundary: this route is unauthenticated by design. Codex starts
//! emitting telemetry before any auth handshake completes, and the bridge proxy
//! already gates `/otel` to loopback origin (bin/bridge/src/proxy/server.rs).
//! Do not add JWT/API-key auth here without coordinating with the bridge.

mod convert;
mod ingest;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use prost::Message;
use systemprompt_database::DbPool;
use systemprompt_logging::LoggingRepository;

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

use ingest::{ingest_logs, ingest_metrics, ingest_traces};

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

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
