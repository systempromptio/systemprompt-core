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
    DispatchError, DispatchInputs, GatewayService, GovernanceDenied, GuardForbidden, PolicyDenied,
    QuotaExceeded, SafetyBlocked,
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
        client_headers,
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
            forward_headers: client_headers.forward,
            identity_headers: client_headers.identity,
        },
    )
    .await
    {
        Ok(resp) => Ok(resp),
        Err(e) => map_dispatch_error(e),
    }
}

const ERROR_TYPE_API: &str = "api_error";
const ERROR_TYPE_PERMISSION: &str = "permission_error";
const ERROR_TYPE_INVALID_REQUEST: &str = "invalid_request_error";

const POLICY_DENIAL_PREFIX: &str = "blocked by systemprompt governance";

/// Renders a governance denial as `400 invalid_request_error`, not `403`.
///
/// Claude Code and other Anthropic-SDK clients treat any 403 on `/v1/messages`
/// as an expired credential: they discard the body and tell the operator to
/// re-login, so the reason never reaches the person who needs it. `400
/// invalid_request_error` is the shape those clients surface verbatim, which is
/// what a policy denial needs — the request was refused on its content, and no
/// amount of re-authenticating will change that.
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_policy_denial(message: &str) -> Response<Body> {
    build_error_response(
        StatusCode::BAD_REQUEST,
        ERROR_TYPE_INVALID_REQUEST,
        &policy_denial_message(message),
    )
}

/// Names the gateway as the source. Without it the deny reads as an upstream
/// Anthropic error, which is exactly how the secret-scan false positive that
/// prompted this was misdiagnosed.
#[must_use]
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn policy_denial_message(message: &str) -> String {
    if message.starts_with(POLICY_DENIAL_PREFIX) {
        return message.to_owned();
    }
    format!("{POLICY_DENIAL_PREFIX}: {message}")
}

/// The Anthropic error `type` conventionally paired with a status code.
#[must_use]
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn error_type_for(status: StatusCode) -> &'static str {
    match status {
        StatusCode::UNAUTHORIZED => "authentication_error",
        StatusCode::FORBIDDEN => ERROR_TYPE_PERMISSION,
        StatusCode::NOT_FOUND => "not_found_error",
        StatusCode::TOO_MANY_REQUESTS => "rate_limit_error",
        s if s.is_client_error() => ERROR_TYPE_INVALID_REQUEST,
        _ => ERROR_TYPE_API,
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
        let mut resp = build_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            error_type_for(StatusCode::TOO_MANY_REQUESTS),
            &quota.message,
        );
        if let Ok(v) = HeaderValue::from_str(&quota.retry_after_seconds.to_string()) {
            resp.headers_mut().insert("retry-after", v);
        }
        return Ok(resp);
    }
    // Why: a guard rejection *is* an authorization failure, so 403 — and the
    // client's prompt to re-authenticate — is the right response here. It is
    // the one case below that a re-login can actually fix.
    if let Some(forbidden) = inner.downcast_ref::<GuardForbidden>() {
        return Ok(build_error_response(
            StatusCode::FORBIDDEN,
            ERROR_TYPE_PERMISSION,
            &forbidden.message,
        ));
    }
    if let Some(denied) = inner.downcast_ref::<GovernanceDenied>() {
        return Ok(build_policy_denial(&denied.message));
    }
    // Why: Claude Code recovers from several provider rejections by matching on
    // the provider's own error wording and retrying without the rejected
    // capability. Re-wrapping the error defeats that even when the status is
    // preserved, so an upstream rejection is relayed exactly as it arrived.
    if let Some(upstream) = inner.downcast_ref::<UpstreamError>()
        && let Some(response) = build_upstream_passthrough(upstream)
    {
        return Ok(response);
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
        return (
            StatusCode::BAD_REQUEST,
            policy_denial_message(&denied.to_string()),
        );
    }
    if let Some(blocked) = e.downcast_ref::<SafetyBlocked>() {
        return (
            StatusCode::BAD_REQUEST,
            policy_denial_message(&blocked.to_string()),
        );
    }
    if let Some(upstream) = e.downcast_ref::<UpstreamError>() {
        return map_upstream_error(upstream);
    }
    (StatusCode::BAD_GATEWAY, e.to_string())
}

fn build_upstream_passthrough(e: &UpstreamError) -> Option<Response<Body>> {
    let UpstreamError::Status {
        status,
        body,
        retry_after,
        request_id,
        ..
    } = e
    else {
        return None;
    };
    if body.is_empty() {
        return None;
    }
    let status = StatusCode::from_u16(*status).ok()?;
    let mut builder = Response::builder()
        .status(status)
        .header("content-type", "application/json");
    if let Some(retry_after) = retry_after {
        builder = builder.header("retry-after", retry_after.as_str());
    }
    if let Some(request_id) = request_id {
        builder = builder.header("x-upstream-request-id", request_id.as_str());
    }
    builder.body(Body::from(body.clone())).ok()
}

pub fn map_upstream_error(e: &UpstreamError) -> (StatusCode, String) {
    let UpstreamError::Status {
        provider,
        status,
        message,
        ..
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
pub fn build_error_response(status: StatusCode, error_type: &str, message: &str) -> Response<Body> {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let body = format!(
        "{{\"type\":\"error\",\"error\":{{\"type\":\"{error_type}\",\"message\":\"{escaped}\"}}}}"
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
