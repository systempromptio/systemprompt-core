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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod convert;

mod ingest;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::convert::{any_value_to_string, attrs_to_json, hex_lower, severity_to_level};
    pub use super::ingest::{ingest_logs, ingest_metrics, ingest_traces};
}

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use prost::Message;

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

use ingest::{ingest_logs, ingest_metrics, ingest_traces};

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub async fn handle(request: Request<Body>) -> Response<Body> {
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

    if let Ok(req) = ExportTraceServiceRequest::decode(body_bytes.as_ref())
        && !req.resource_spans.is_empty()
    {
        ingest_traces(req);
        return accepted();
    }
    if let Ok(req) = ExportLogsServiceRequest::decode(body_bytes.as_ref())
        && !req.resource_logs.is_empty()
    {
        ingest_logs(req);
        return accepted();
    }
    if let Ok(req) = ExportMetricsServiceRequest::decode(body_bytes.as_ref())
        && !req.resource_metrics.is_empty()
    {
        ingest_metrics(&req);
        return accepted();
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
