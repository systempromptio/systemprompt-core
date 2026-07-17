//! Gateway message dispatch: route resolution and upstream invocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;

use crate::services::gateway::audit::GatewayRequestContext;
use crate::services::gateway::protocol::inbound::InboundAdapter;
use crate::services::gateway::protocol::outbound::UpstreamError;
use crate::services::gateway::service::{
    DispatchError, DispatchInputs, GatewayService, PolicyDenied, QuotaExceeded, SafetyBlocked,
};

use super::RequestContext;
use super::extract::PreparedRequest;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub struct RejectionError {
    pub status: StatusCode,
    pub message: String,
    pub persist: bool,
}

pub(super) async fn dispatch_to_provider(
    rc: &RequestContext<'_>,
    inbound: Arc<dyn InboundAdapter>,
    prepared: PreparedRequest,
) -> Result<Response<Body>, RejectionError> {
    let PreparedRequest {
        principal,
        body_bytes,
        gateway_request,
        provider,
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
    } = prepared;

    let max_tokens = gateway_request.max_tokens;
    let is_streaming = gateway_request.stream;

    let gateway_ctx = GatewayRequestContext {
        ai_request_id: rc.ai_request_id.clone(),
        user_id: principal.user_id().clone(),
        session_id: Some(session_id),
        context_id,
        gateway_conversation_id: Some(gateway_conversation_id),
        trace_id: Some(principal.trace_id().clone()),
        provider,
        requested_model: Some(gateway_request.model.clone()),
        model: upstream_model,
        max_tokens: Some(max_tokens),
        is_streaming,
        wire_protocol: inbound.wire_name().to_owned(),
    };

    let gateway_config = rc
        .profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .ok_or_else(|| RejectionError {
            status: StatusCode::NOT_FOUND,
            message: "Gateway not enabled".to_owned(),
            persist: true,
        })?;

    match GatewayService::dispatch(
        gateway_config,
        &rc.profile.providers,
        rc.ctx.db_pool(),
        DispatchInputs {
            request: gateway_request,
            raw_body: body_bytes,
            ctx: gateway_ctx,
            inbound,
        },
    )
    .await
    {
        Ok(resp) => Ok(resp),
        Err(e) => map_dispatch_error(e),
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn map_dispatch_error(e: DispatchError) -> Result<Response<Body>, RejectionError> {
    let (persist, inner) = match e {
        DispatchError::PreAudit(inner) => (true, inner),
        DispatchError::Recorded(inner) => (false, inner),
    };
    if let Some(quota) = inner.downcast_ref::<QuotaExceeded>() {
        let mut resp = build_error_response(StatusCode::TOO_MANY_REQUESTS, &quota.message);
        if let Ok(v) = HeaderValue::from_str(&quota.retry_after_seconds.to_string()) {
            resp.headers_mut().insert("retry-after", v);
        }
        return Ok(resp);
    }
    let (status, message) = classify_dispatch_error(&inner);
    Err(RejectionError {
        status,
        message,
        persist,
    })
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn classify_dispatch_error(e: &anyhow::Error) -> (StatusCode, String) {
    if let Some(denied) = e.downcast_ref::<PolicyDenied>() {
        return (StatusCode::FORBIDDEN, denied.to_string());
    }
    if let Some(blocked) = e.downcast_ref::<SafetyBlocked>() {
        return (StatusCode::FORBIDDEN, blocked.to_string());
    }
    if let Some(upstream) = e.downcast_ref::<UpstreamError>() {
        return map_upstream_error(upstream);
    }
    (StatusCode::BAD_GATEWAY, e.to_string())
}

pub fn map_upstream_error(e: &UpstreamError) -> (StatusCode, String) {
    let UpstreamError::Status {
        provider,
        status,
        message,
    } = e
    else {
        return (
            StatusCode::BAD_GATEWAY,
            "upstream provider unreachable".to_owned(),
        );
    };
    let mapped = match *status {
        400 | 404 | 422 => StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST),
        429 => StatusCode::TOO_MANY_REQUESTS,
        408 | 504 => StatusCode::GATEWAY_TIMEOUT,
        _ => StatusCode::BAD_GATEWAY,
    };
    if mapped.is_server_error() {
        (mapped, "upstream provider error".to_owned())
    } else {
        (
            mapped,
            format!("{provider} rejected the request: {message}"),
        )
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        "{{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}"
    );
    match Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
    {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!(error = %e, status = %status, "Failed to build gateway error response");
            internal_error_response()
        },
    }
}

fn internal_error_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"error":"internal"}"#))
        .unwrap_or_else(|_| {
            let mut fallback = Response::new(Body::from(r#"{"error":"internal"}"#));
            *fallback.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            fallback
        })
}
