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
    DispatchError, GovernanceDenied, GuardForbidden, PolicyDenied, QuotaExceeded, SafetyBlocked,
};

fn rejection(e: DispatchError) -> (StatusCode, String, bool) {
    let err = map_dispatch_error(e).expect_err("this error must not render a response");
    (err.status, err.message, err.persist)
}

#[test]
fn a_policy_denial_is_a_400_carrying_the_policy_reason() {
    let (status, message) = classify_dispatch_error(&anyhow::Error::new(PolicyDenied(
        "model not allowed".to_owned(),
    )));

    // Why: 403 makes Anthropic-SDK clients report an auth failure and discard
    // the body, so the operator never learns which policy refused them.
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        message,
        "blocked by systemprompt governance: model not allowed"
    );
}

#[test]
fn a_safety_block_is_a_400_carrying_the_scanner_message() {
    let (status, message) = classify_dispatch_error(&anyhow::Error::new(SafetyBlocked {
        category: "self-harm".to_owned(),
        message: "blocked by safety scanner".to_owned(),
    }));

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        message,
        "blocked by systemprompt governance: blocked by safety scanner"
    );
}

#[test]
fn an_upstream_status_is_delegated_to_the_upstream_mapping() {
    let (status, message) = classify_dispatch_error(&anyhow::Error::new(UpstreamError::Status {
        provider: "anthropic".to_owned(),
        status: 429,
        message: "slow down".to_owned(),
        body: bytes::Bytes::new(),
        retry_after: None,
        request_id: None,
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

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(message, "blocked by systemprompt governance: denied");
}

#[tokio::test]
async fn a_governance_denial_renders_an_envelope_the_client_will_show_the_operator() {
    let response = map_dispatch_error(DispatchError::Recorded(anyhow::Error::new(
        GovernanceDenied {
            policy: "secret_scan".to_owned(),
            message: "secret detected: High-entropy token at prompt.text".to_owned(),
        },
    )))
    .expect("a governance denial renders a response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("the error body is small and fully buffered");
    let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON envelope");

    // Why: `invalid_request_error` is the type Anthropic-SDK clients surface
    // verbatim. `api_error` on a 403 is what made this deny render as
    // "Please run /login" with the reason discarded.
    assert_eq!(parsed["error"]["type"], "invalid_request_error");
    let message = parsed["error"]["message"]
        .as_str()
        .expect("the message is a string");
    assert!(
        message.starts_with("blocked by systemprompt governance:"),
        "{message}"
    );
    assert!(message.contains("prompt.text"), "{message}");
}

#[test]
fn a_guard_rejection_stays_a_403_because_re_authenticating_can_fix_it() {
    let response = map_dispatch_error(DispatchError::Recorded(anyhow::Error::new(
        GuardForbidden {
            message: "no gateway scope".to_owned(),
        },
    )))
    .expect("a guard rejection renders a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// The verbatim upstream passthrough.
//
// Every test above hands `map_dispatch_error` an empty upstream body, which is
// the one input that switches passthrough *off* — so the relay path below,
// which is what actually reaches the client on a provider rejection, went
// unexercised. These drive it.

fn upstream(
    status: u16,
    body: &str,
    retry_after: Option<&str>,
    request_id: Option<&str>,
) -> UpstreamError {
    UpstreamError::Status {
        provider: "anthropic".to_owned(),
        status,
        message: "upstream said no".to_owned(),
        body: bytes::Bytes::from(body.to_owned()),
        retry_after: retry_after.map(ToOwned::to_owned),
        request_id: request_id.map(ToOwned::to_owned),
    }
}

fn relayed(e: UpstreamError) -> axum::response::Response<axum::body::Body> {
    map_dispatch_error(DispatchError::Recorded(anyhow::Error::new(e)))
        .expect("an upstream rejection with a body must be relayed, not re-wrapped")
}

async fn body_string(response: axum::response::Response<axum::body::Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read the relayed body");
    String::from_utf8(bytes.to_vec()).expect("the relayed body is utf-8")
}

// Why: the source records that Claude Code recovers from several provider
// rejections by matching on the provider's own error wording and retrying
// without the rejected capability. Re-wrapping the message defeats that even
// when the status is preserved, so the body has to arrive byte-for-byte.
#[tokio::test]
async fn an_upstream_rejection_is_relayed_byte_for_byte() {
    let original = r#"{"type":"error","error":{"type":"invalid_request_error","message":"tool x is not supported"}}"#;

    let response = relayed(upstream(400, original, None, None));

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "the upstream status is kept"
    );
    assert_eq!(
        body_string(response).await,
        original,
        "the client matches on the provider's own wording; any rewrite breaks its recovery"
    );
}

#[tokio::test]
async fn a_relayed_rejection_is_announced_as_json() {
    let response = relayed(upstream(429, r#"{"error":"rate limited"}"#, None, None));

    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json"),
        "a relayed body is JSON and must say so, or the client will not parse it"
    );
}

// Why: retry-after is the only thing telling the client how long to wait. It
// lives on the upstream response, so dropping it during the relay turns a
// recoverable rate-limit into a client-side guess.
#[tokio::test]
async fn the_upstream_retry_after_survives_the_relay() {
    let response = relayed(upstream(429, r#"{"error":"slow down"}"#, Some("30"), None));

    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("30"),
        "the upstream's own backoff hint must reach the client"
    );
}

// Why: the upstream request id is what makes a provider-side incident
// traceable. It is the only link between our rejection and their logs.
#[tokio::test]
async fn the_upstream_request_id_survives_the_relay() {
    let response = relayed(upstream(
        500,
        r#"{"error":"internal"}"#,
        None,
        Some("req_abc123"),
    ));

    assert_eq!(
        response
            .headers()
            .get("x-upstream-request-id")
            .and_then(|v| v.to_str().ok()),
        Some("req_abc123"),
        "without this a provider-side failure cannot be traced back to their logs"
    );
}

#[tokio::test]
async fn absent_upstream_headers_are_not_invented() {
    let response = relayed(upstream(400, r#"{"error":"bad"}"#, None, None));

    assert!(
        response.headers().get("retry-after").is_none(),
        "a retry-after the upstream never sent would be a fabricated backoff"
    );
    assert!(
        response.headers().get("x-upstream-request-id").is_none(),
        "an invented request id would point at nothing in the provider's logs"
    );
}

// Why: with no body there is nothing to relay, so the error must fall through
// to classification rather than producing an empty 200-shaped response. This is
// the branch every other test in this file happens to take.
#[test]
fn an_upstream_rejection_with_no_body_falls_through_to_classification() {
    let (status, message, _) = rejection(DispatchError::Recorded(anyhow::Error::new(upstream(
        429, "", None, None,
    ))));

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        message.contains("anthropic"),
        "with nothing to relay the client gets our classified message: {message}"
    );
}

// Why: `status` is a u16 carried from the wire, so it can hold a value outside
// the 100-999 range HTTP defines. Relaying it would emit a malformed response,
// so the conversion is fallible and the error falls through to classification.
//
// The boundary is wider than it looks: 999 is representable and *is* relayed.
// Only a value HTTP cannot express reaches this branch.
#[test]
fn an_upstream_status_outside_the_http_range_falls_through_to_classification() {
    let (status, _, _) = rejection(DispatchError::Recorded(anyhow::Error::new(upstream(
        1000,
        r#"{"error":"nonsense"}"#,
        None,
        None,
    ))));

    assert_eq!(
        status,
        StatusCode::BAD_GATEWAY,
        "a status HTTP cannot express must not be relayed"
    );
}

#[tokio::test]
async fn a_nonstandard_but_representable_status_is_still_relayed() {
    let response = relayed(upstream(599, r#"{"error":"vendor specific"}"#, None, None));

    assert_eq!(
        response.status().as_u16(),
        599,
        "a status HTTP can express is the provider's to choose, not ours to normalise"
    );
}
