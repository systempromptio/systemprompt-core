//! Dispatch-error classification and the JSON error responses the gateway
//! returns to clients, including verbatim upstream passthrough.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;

use crate::services::gateway::image_fetch::ImageFetchFailed;
use crate::services::gateway::protocol::outbound::UpstreamError;
use crate::services::gateway::service::{
    DispatchError, GovernanceDenied, GuardForbidden, PolicyDenied, QuotaExceeded, SafetyBlocked,
};

use super::RejectionError;

const ERROR_TYPE_API: &str = "api_error";
const ERROR_TYPE_PERMISSION: &str = "permission_error";
const ERROR_TYPE_INVALID_REQUEST: &str = "invalid_request_error";

const POLICY_DENIAL_PREFIX: &str = "blocked by systemprompt governance";

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
    // Why: the image is part of the prompt. Degrading to text and answering
    // anyway is the defect this path exists to remove, so the request fails and
    // says which URL failed and whether the caller can fix it.
    if let Some(image) = inner.downcast_ref::<ImageFetchFailed>() {
        let status = if image.caller_fault {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::BAD_GATEWAY
        };
        return Ok(build_error_response(
            status,
            error_type_for(status),
            &image.to_string(),
        ));
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
