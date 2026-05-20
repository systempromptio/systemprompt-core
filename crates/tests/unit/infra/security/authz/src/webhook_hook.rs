use std::sync::Arc;
use std::time::Duration;

use systemprompt_identifiers::TraceId;
use systemprompt_security::authz::audit::NullAuditSink;
use systemprompt_security::authz::hook::DenyAllHook;
use systemprompt_security::authz::{
    AllowAllHook, AuthzDecision, AuthzDecisionHook, AuthzRequest, EntityKind, WebhookHook,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use systemprompt_test_fixtures::fixture_user_id;

fn fixture() -> AuthzRequest {
    AuthzRequest {
        entity_type: EntityKind::GatewayRoute,
        entity_id: "claude-3".into(),
        user_id: fixture_user_id(),
        roles: vec!["eng".into()],
        department: "platform".into(),
        trace_id: TraceId::new("trace-1"),
        context: serde_json::json!({"model": "claude-3"}),
        act_chain: Vec::new(),
    }
}

#[tokio::test]
async fn webhook_returns_allow_decision() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/authz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "decision": "allow",
        })))
        .mount(&server)
        .await;

    let hook = WebhookHook::new(
        format!("{}/authz", server.uri()),
        Duration::from_secs(2),
        Arc::new(NullAuditSink),
    )
    .expect("build webhook hook");

    let decision = hook.evaluate(fixture()).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn webhook_returns_deny_decision() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/authz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "decision": "deny",
            "reason": "policy violation",
            "policy": "test_policy",
        })))
        .mount(&server)
        .await;

    let hook = WebhookHook::new(
        format!("{}/authz", server.uri()),
        Duration::from_secs(2),
        Arc::new(NullAuditSink),
    )
    .expect("build webhook hook");

    let decision = hook.evaluate(fixture()).await;
    match decision {
        AuthzDecision::Deny { reason, policy } => {
            assert_eq!(reason, "policy violation");
            assert_eq!(policy, "test_policy");
        },
        AuthzDecision::Allow => panic!("expected deny decision"),
    }
}

#[tokio::test]
async fn webhook_transport_failure_denies() {
    let hook = WebhookHook::new(
        "http://127.0.0.1:1/authz".to_string(),
        Duration::from_millis(50),
        Arc::new(NullAuditSink),
    )
    .expect("build webhook hook");

    let decision = hook.evaluate(fixture()).await;
    assert!(
        matches!(decision, AuthzDecision::Deny { .. }),
        "transport failures must fail closed",
    );
}

#[tokio::test]
async fn allow_all_hook_always_allows() {
    let hook = AllowAllHook::null();
    let decision = hook.evaluate(fixture()).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn deny_all_hook_always_denies() {
    let hook = DenyAllHook::null();
    let decision = hook.evaluate(fixture()).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}
