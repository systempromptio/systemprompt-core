//! OTLP telemetry ingest endpoint.
//!
//! [`handle`] authenticates the caller with the same gateway credential the
//! inference routes accept (bridge JWT, API key or execution capability, plus
//! the attested `x-session-id`) and then hands the body to
//! [`ingest_envelope`], which decodes a protobuf OTLP envelope (traces, logs,
//! or metrics) and persists spans and log records to the logging repository
//! (see the `ingest` submodule); metrics are only summarised. Once
//! authenticated the route always responds `202 Accepted`, swallowing decode
//! and persist failures so a misbehaving emitter cannot stall.
//!
//! Trust boundary: the bridge proxy forwards `/otel` only from loopback callers
//! presenting its secret and attaches its own gateway bearer, so an emitter
//! never needs a credential of its own; a request reaching this route without
//! one is refused with 401 rather than written to the log store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod convert;

pub mod ingest;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use prost::Message;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

use super::messages::auth::authenticate;
use super::messages::extract::headers::{extract_credential, require_session_id};
use crate::services::gateway::GatewayRepositories;
use crate::services::middleware::JwtContextExtractor;
use ingest::{ingest_logs, ingest_metrics, ingest_traces};

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    repos: Arc<GatewayRepositories>,
    request: Request<Body>,
) -> Response<Body> {
    let Some(credential) = extract_credential(request.headers()) else {
        return (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential",
        )
            .into_response();
    };
    let session_id = match require_session_id(request.headers()) {
        Ok(session_id) => session_id,
        Err(rejection) => return rejection.into_response(),
    };
    let principal = match authenticate(
        &credential,
        &session_id,
        &jwt_extractor,
        &ctx,
        &repos.execution_capabilities,
    )
    .await
    {
        Ok(principal) => principal,
        Err(rejection) => return rejection.into_response(),
    };
    if let Err(rejection) = principal.enforce_session_binding(&session_id) {
        return rejection.into_response();
    }
    ingest_envelope(request).await
}

pub async fn ingest_envelope(request: Request<Body>) -> Response<Body> {
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
