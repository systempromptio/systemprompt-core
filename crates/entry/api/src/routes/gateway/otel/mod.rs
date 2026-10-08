//! OTLP telemetry ingest endpoint.
//!
//! [`handle`] authenticates the caller with the same gateway credential the
//! inference routes accept (bridge JWT or API key, plus
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
pub mod json;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use prost::Message;
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_runtime::AppContext;

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;

use super::bridge_error::BridgeError;
use super::messages::auth::authenticate;
use super::messages::extract::headers::{extract_credential, require_session_id};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;
use ingest::{ingest_logs, ingest_metrics, ingest_traces};

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request<Body>,
) -> Response<Body> {
    let Some(credential) = extract_credential(request.headers()) else {
        return ApiHttpError::from(BridgeError::MissingCredential).into_response();
    };
    let session_id = match require_session_id(request.headers()) {
        Ok(session_id) => session_id,
        Err(rejection) => return rejection.into_response(),
    };
    let principal = match authenticate(&credential, &session_id, &jwt_extractor, &ctx).await {
        Ok(principal) => principal,
        Err(rejection) => return rejection.into_response(),
    };
    if let Err(rejection) = principal.enforce_session_binding(&session_id) {
        return rejection.into_response();
    }
    ingest_with_identity(request, Some((principal.user_id().clone(), session_id))).await
}

pub async fn ingest_envelope(request: Request<Body>) -> Response<Body> {
    ingest_with_identity(request, None).await
}

async fn ingest_with_identity(
    request: Request<Body>,
    identity: Option<(UserId, SessionId)>,
) -> Response<Body> {
    let signal = request
        .uri()
        .path()
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_owned();
    let is_json = request
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|kind| kind.trim() == "application/json")
        });
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

    if is_json && signal != "metrics" && signal != "traces" {
        ingest_json_logs(&body_bytes, identity.as_ref());
        return accepted();
    }
    ingest_protobuf(&signal, &body_bytes, identity.as_ref())
}

fn ingest_json_logs(body_bytes: &[u8], identity: Option<&(UserId, SessionId)>) {
    match json::decode_logs(body_bytes) {
        Ok(mut req) if !req.resource_logs.is_empty() => {
            for item in &mut req.resource_logs {
                bind_identity(&mut item.resource, identity);
            }
            count_export("logs");
            ingest_logs(req);
        },
        _ => tracing::warn!(bytes = body_bytes.len(), "otel: invalid JSON log export"),
    }
}

fn ingest_protobuf(
    signal: &str,
    body_bytes: &[u8],
    identity: Option<&(UserId, SessionId)>,
) -> Response<Body> {
    if !matches!(signal, "logs" | "metrics")
        && let Ok(mut req) = ExportTraceServiceRequest::decode(body_bytes)
        && !req.resource_spans.is_empty()
    {
        for item in &mut req.resource_spans {
            bind_identity(&mut item.resource, identity);
        }
        count_export("traces");
        ingest_traces(req);
        return accepted();
    }
    if !matches!(signal, "traces" | "metrics")
        && let Ok(mut req) = ExportLogsServiceRequest::decode(body_bytes)
        && !req.resource_logs.is_empty()
    {
        for item in &mut req.resource_logs {
            bind_identity(&mut item.resource, identity);
        }
        count_export("logs");
        ingest_logs(req);
        return accepted();
    }
    if !matches!(signal, "traces" | "logs")
        && let Ok(req) = ExportMetricsServiceRequest::decode(body_bytes)
        && !req.resource_metrics.is_empty()
    {
        count_export("metrics");
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


fn bind_identity(
    resource: &mut Option<opentelemetry_proto::tonic::resource::v1::Resource>,
    identity: Option<&(UserId, SessionId)>,
) {
    use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value};
    let resource = resource.get_or_insert_default();
    resource.attributes.retain(|kv| {
        !matches!(
            kv.key.as_str(),
            "systemprompt.user.id" | "systemprompt.session.id" | "enduser.id"
        )
    });
    if let Some((user, session)) = identity {
        for (key, value) in [
            ("systemprompt.user.id", user.as_str()),
            ("systemprompt.session.id", session.as_str()),
        ] {
            resource.attributes.push(KeyValue {
                key: key.to_owned(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(value.to_owned())),
                }),
                ..Default::default()
            });
        }
    }
}

fn count_export(signal: &'static str) {
    metrics::counter!("gateway_desktop_telemetry_exports_total", "signal" => signal).increment(1);
}
