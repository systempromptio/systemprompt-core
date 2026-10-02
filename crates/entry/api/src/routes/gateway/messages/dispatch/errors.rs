//! Dispatch-error classification and the JSON error responses the gateway
//! returns to clients, including verbatim upstream passthrough.
//!
//! [`map_dispatch_error`] is the one place a [`GatewayError`] becomes a
//! provider-shaped answer: a rendered envelope for the failures that carry
//! their own headers or body, or a [`RejectionError`] the handler renders
//! through the inbound wire. Classification is a `match` on the variant; a 5xx
//! never carries the error's own text, and every rejection keeps the error as
//! its logged cause, so a fixed public message never costs the diagnosis.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::services::gateway::protocol::outbound::UpstreamError;
use crate::services::gateway::service::{DispatchError, GatewayError, upstream_status};

use super::RejectionError;

const ERROR_TYPE_PERMISSION: &str = "permission_error";
const ERROR_TYPE_INVALID_REQUEST: &str = "invalid_request_error";

const POLICY_DENIAL_PREFIX: &str = "blocked by systemprompt governance";
const PROMPT_REPAIR_ACTION: &str = "Remove secret-bearing content, correct system instructions, \
                                    or shorten the conversation before retrying";
const UNSERVABLE_MODEL_MESSAGE: &str = "The requested model is not served by this gateway";
const IMAGE_FETCH_FAILED_MESSAGE: &str = "An image URL in the request could not be fetched";

pub fn build_policy_denial(message: &str) -> Response<Body> {
    build_error_response(
        StatusCode::BAD_REQUEST,
        ERROR_TYPE_INVALID_REQUEST,
        &policy_denial_message(message),
    )
}

#[must_use]
pub fn policy_denial_message(message: &str) -> String {
    if message.starts_with(POLICY_DENIAL_PREFIX) {
        return message.to_owned();
    }
    format!("{POLICY_DENIAL_PREFIX}: {message}")
}

pub fn map_dispatch_error(e: DispatchError) -> Result<Response<Body>, RejectionError> {
    let (persist, error) = match e {
        DispatchError::PreAudit(error) => (true, error),
        DispatchError::Recorded(error) => (false, error),
    };
    if let Some(response) = render_error(&error) {
        return Ok(response);
    }
    Err(classify_dispatch_error(error).with_persist(persist))
}

fn render_error(error: &GatewayError) -> Option<Response<Body>> {
    match error {
        GatewayError::Quota(quota) => Some(with_retry_after(
            build_error_response(error.status(), error.error_type(), &quota.message),
            quota.retry_after_seconds,
        )),
        GatewayError::GuardUnavailable(unavailable) => Some(with_retry_after(
            build_error_response(error.status(), error.error_type(), &unavailable.message),
            unavailable.retry_after_seconds,
        )),
        GatewayError::GuardForbidden(forbidden) => Some(build_error_response(
            error.status(),
            ERROR_TYPE_PERMISSION,
            &forbidden.message,
        )),
        GatewayError::PromptRepair(repair) => {
            Some(build_prompt_repair(&repair.message, &repair.locations))
        },
        GatewayError::Governance(denied) => Some(build_policy_denial(&denied.message)),
        GatewayError::ImageFetch(_) => {
            let rejection = classify_dispatch_error_ref(error);
            Some(build_error_response(
                rejection.status,
                error.error_type(),
                rejection.public_message(),
            ))
        },
        // Why: Claude Code matches provider error wording to retry without
        // rejected capabilities.
        GatewayError::Upstream(upstream) => build_upstream_passthrough(upstream),
        GatewayError::PolicyDenied(_)
        | GatewayError::PolicyUnavailable(_)
        | GatewayError::Safety(_)
        | GatewayError::MissingPricing(_)
        | GatewayError::UpstreamTarget(_)
        | GatewayError::NoRoute { .. }
        | GatewayError::UndeclaredProvider { .. }
        | GatewayError::NoAdapter { .. }
        | GatewayError::MissingSession
        | GatewayError::Internal { .. } => None,
    }
}

pub fn classify_dispatch_error(error: GatewayError) -> RejectionError {
    classify_dispatch_error_ref(&error).with_cause(error)
}

fn classify_dispatch_error_ref(error: &GatewayError) -> RejectionError {
    let status = error.status();
    match error {
        GatewayError::PolicyDenied(denied) => {
            RejectionError::client(status, policy_denial_message(&denied.0))
        },
        GatewayError::Governance(denied) => {
            RejectionError::client(status, policy_denial_message(&denied.message))
        },
        GatewayError::PromptRepair(repair) => {
            RejectionError::client(status, policy_denial_message(&repair.message))
        },
        GatewayError::Safety(blocked) => {
            RejectionError::client(status, policy_denial_message(&blocked.message))
        },
        GatewayError::Quota(quota) => RejectionError::client(status, quota.message.clone()),
        GatewayError::GuardForbidden(forbidden) => {
            RejectionError::client(status, forbidden.message.clone())
        },
        GatewayError::GuardUnavailable(_) => {
            RejectionError::server(status, "request guard unavailable")
        },
        GatewayError::PolicyUnavailable(_) => {
            RejectionError::server(status, "gateway policy unavailable")
        },
        GatewayError::ImageFetch(image) if !status.is_server_error() => {
            RejectionError::client(status, image.to_string())
        },
        GatewayError::ImageFetch(_) => RejectionError::server(status, IMAGE_FETCH_FAILED_MESSAGE),
        GatewayError::Upstream(upstream) => map_upstream_error(upstream),
        GatewayError::MissingPricing(_)
        | GatewayError::NoRoute { .. }
        | GatewayError::UndeclaredProvider { .. }
        | GatewayError::NoAdapter { .. } => {
            RejectionError::client(status, UNSERVABLE_MODEL_MESSAGE)
        },
        GatewayError::UpstreamTarget(_) if !status.is_server_error() => {
            RejectionError::client(status, UNSERVABLE_MODEL_MESSAGE)
        },
        GatewayError::UpstreamTarget(_) => {
            RejectionError::server(status, "upstream credential unavailable")
        },
        GatewayError::MissingSession => {
            RejectionError::server(status, "dispatch without an authenticated session")
        },
        GatewayError::Internal { context, .. } => RejectionError::server(status, context),
    }
}

fn with_retry_after(mut response: Response<Body>, seconds: i32) -> Response<Body> {
    if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
        response.headers_mut().insert("retry-after", value);
    }
    response
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
    builder.body(Body::from(bytes::Bytes::clone(body))).ok()
}

pub fn map_upstream_error(e: &UpstreamError) -> RejectionError {
    let mapped = upstream_status(e);
    match e {
        UpstreamError::Status {
            provider, message, ..
        } if !mapped.is_server_error() => RejectionError::client(
            mapped,
            format!("{provider} rejected the request: {message}"),
        ),
        UpstreamError::Status { .. } => RejectionError::server(mapped, "upstream provider error"),
        UpstreamError::Transport { .. } => {
            RejectionError::server(mapped, "upstream provider unreachable")
        },
    }
}

pub fn build_error_response(status: StatusCode, error_type: &str, message: &str) -> Response<Body> {
    (
        status,
        axum::Json(serde_json::json!({
            "type": "error",
            "error": { "type": error_type, "message": message },
        })),
    )
        .into_response()
}

fn build_prompt_repair(message: &str, locations: &[String]) -> Response<Body> {
    let body = serde_json::json!({
        "type": "error",
        "error": {
            "type": ERROR_TYPE_INVALID_REQUEST,
            "message": policy_denial_message(message),
            "recovery": {
                "code": "prompt_repair_required",
                "locations": locations,
                "retryable": false,
                "action": PROMPT_REPAIR_ACTION,
            }
        }
    });
    (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
}
