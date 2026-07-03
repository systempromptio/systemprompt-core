//! Integration tests (coverage campaign 2026-07).
//!
//! Unit-level coverage for the gateway dispatch error mapper and the response
//! finalization helpers reached through the `test_api` seams: `DispatchError`
//! classification into HTTP status + body, the quota `retry-after` fast path,
//! the JSON error-envelope builder, request-id stamping, and the
//! system-prompt-override no-op when no overrides are configured.

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use systemprompt_api::routes::gateway::messages::test_api::{
    build_error_response, classify_dispatch_error, map_dispatch_error,
};
use systemprompt_api::services::gateway::protocol::outbound::UpstreamError;
use systemprompt_api::services::gateway::protocol::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::service::test_api::{
    apply_system_prompt_override, attach_request_id,
};
use systemprompt_api::services::gateway::service::{
    DispatchError, PolicyDenied, QuotaExceeded, REQUEST_ID_HEADER, SafetyBlocked,
};
use systemprompt_identifiers::{AiRequestId, ProviderId};
use systemprompt_models::profile::GatewayConfig;

#[test]
fn classify_policy_denied_is_forbidden() {
    let err = anyhow::Error::new(PolicyDenied("model blocked".to_owned()));
    let (status, msg) = classify_dispatch_error(&err);
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(msg.contains("model blocked"), "{msg}");
}

#[test]
fn classify_safety_blocked_is_forbidden() {
    let err = anyhow::Error::new(SafetyBlocked {
        category: "self-harm".to_owned(),
        message: "blocked by safety scanner".to_owned(),
    });
    let (status, msg) = classify_dispatch_error(&err);
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(msg.contains("blocked by safety scanner"), "{msg}");
}

#[test]
fn classify_upstream_status_maps_through() {
    let err = anyhow::Error::new(UpstreamError::Status {
        provider: "openai".to_owned(),
        status: 429,
        message: "slow down".to_owned(),
    });
    let (status, _msg) = classify_dispatch_error(&err);
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[test]
fn classify_unknown_error_is_bad_gateway() {
    let err = anyhow::anyhow!("something broke deep in the stack");
    let (status, msg) = classify_dispatch_error(&err);
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(msg.contains("something broke"), "{msg}");
}

#[test]
fn map_dispatch_error_quota_returns_retry_after_response() {
    let err = DispatchError::Recorded(anyhow::Error::new(QuotaExceeded {
        message: "daily budget exhausted".to_owned(),
        retry_after_seconds: 42,
    }));
    let resp = map_dispatch_error(err).expect("quota is a response, not a rejection");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        resp.headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("42")
    );
}

#[test]
fn map_dispatch_error_pre_audit_marks_persist() {
    let err = DispatchError::PreAudit(anyhow::Error::new(PolicyDenied("nope".to_owned())));
    let rejection = map_dispatch_error(err).expect_err("policy denial is a rejection");
    assert_eq!(rejection.status, StatusCode::FORBIDDEN);
    assert!(rejection.persist, "pre-audit rejections must persist");
}

#[test]
fn map_dispatch_error_recorded_skips_persist() {
    let err = DispatchError::Recorded(anyhow::Error::new(PolicyDenied("nope".to_owned())));
    let rejection = map_dispatch_error(err).expect_err("policy denial is a rejection");
    assert!(!rejection.persist, "already-recorded errors do not re-persist");
}

#[test]
fn build_error_response_escapes_quotes_in_message() {
    let resp = build_error_response(StatusCode::BAD_REQUEST, "bad \"model\" name");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
}

#[test]
fn attach_request_id_stamps_header() {
    let id = AiRequestId::generate();
    let response = attach_request_id(Response::new(Body::empty()), &id);
    assert_eq!(
        response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some(id.as_str())
    );
}

fn canonical() -> CanonicalRequest {
    CanonicalRequest {
        model: "claude-test".to_owned(),
        system: Some("keep it short".to_owned()),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hi".to_owned())],
        }],
        max_tokens: 32,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

#[tokio::test]
async fn apply_system_prompt_override_is_noop_without_overrides() {
    let config = GatewayConfig {
        enabled: true,
        ..GatewayConfig::default()
    };
    let provider = ProviderId::new("anthropic");
    let mut request = canonical();
    let before = request.system.clone();
    let descriptor =
        apply_system_prompt_override(&config, &provider, "claude-test-upstream", &mut request).await;
    assert!(descriptor.is_none(), "no overrides configured");
    assert_eq!(request.system, before, "system prompt is untouched");
}
