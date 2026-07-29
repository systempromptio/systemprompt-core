//! Dispatch-failure classification for the gateway `/messages` handler.
//!
//! `classify_dispatch_error` picks the client-facing status by downcasting the
//! opaque `anyhow::Error` the gateway service returns; `map_dispatch_error`
//! wraps it, and decides both whether a quota failure becomes a rendered 429
//! (rather than a rejection) and whether the rejection gets persisted for
//! audit. These are the only places the caller's status is decided, so every
//! arm is pinned here.

use axum::http::StatusCode;
use systemprompt_api::routes::gateway::messages::test_api::{
    classify_dispatch_error, map_dispatch_error,
};
use systemprompt_api::services::gateway::protocol::outbound::UpstreamError;
use systemprompt_api::services::gateway::service::{
    DispatchError, GuardForbidden, PolicyDenied, QuotaExceeded, SafetyBlocked,
};

fn rejection(e: DispatchError) -> (StatusCode, String, bool) {
    let err = map_dispatch_error(e).expect_err("this error must not render a response");
    (err.status, err.message, err.persist)
}

#[test]
fn a_policy_denial_is_a_403_carrying_the_policy_reason() {
    let (status, message) = classify_dispatch_error(&anyhow::Error::new(PolicyDenied(
        "model not allowed".to_owned(),
    )));

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message, "model not allowed");
}

#[test]
fn a_safety_block_is_a_403_carrying_the_scanner_message() {
    let (status, message) = classify_dispatch_error(&anyhow::Error::new(SafetyBlocked {
        category: "self-harm".to_owned(),
        message: "blocked by safety scanner".to_owned(),
    }));

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message, "blocked by safety scanner");
}

#[test]
fn an_upstream_status_is_delegated_to_the_upstream_mapping() {
    let (status, message) = classify_dispatch_error(&anyhow::Error::new(UpstreamError::Status {
        provider: "anthropic".to_owned(),
        status: 429,
        message: "slow down".to_owned(),
    }));

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(message.contains("anthropic"), "{message}");
}

#[test]
fn an_unrecognised_error_collapses_to_502() {
    let (status, message) = classify_dispatch_error(&anyhow::anyhow!("connection reset"));

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(message, "connection reset");
}

#[test]
fn a_quota_failure_renders_a_429_response_with_retry_after() {
    let response = map_dispatch_error(DispatchError::PreAudit(anyhow::Error::new(QuotaExceeded {
        message: "monthly budget exhausted".to_owned(),
        retry_after_seconds: 90,
    })))
    .expect("a quota failure renders a response rather than a rejection");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .expect("retry-after must be advertised"),
        "90"
    );
}

#[test]
fn a_guard_forbidden_renders_a_403_response_without_retry_after() {
    let response = map_dispatch_error(DispatchError::Recorded(anyhow::Error::new(
        GuardForbidden {
            message: "your plan does not include this model".to_owned(),
        },
    )))
    .expect("an entitlement denial renders a response rather than a rejection");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(
        response.headers().get("retry-after").is_none(),
        "an entitlement denial is not retryable and must not advertise retry-after"
    );
}

#[test]
fn a_pre_audit_failure_is_persisted_but_a_recorded_one_is_not() {
    let (_, _, pre_audit_persists) = rejection(DispatchError::PreAudit(anyhow::anyhow!("boom")));
    let (_, _, recorded_persists) = rejection(DispatchError::Recorded(anyhow::anyhow!("boom")));

    assert!(
        pre_audit_persists,
        "a failure before the audit row exists must be written"
    );
    assert!(
        !recorded_persists,
        "a failure the gateway already recorded must not be double-written"
    );
}

#[test]
fn map_dispatch_error_preserves_the_classified_status() {
    let (status, message, _) = rejection(DispatchError::Recorded(anyhow::Error::new(
        PolicyDenied("denied".to_owned()),
    )));

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message, "denied");
}
