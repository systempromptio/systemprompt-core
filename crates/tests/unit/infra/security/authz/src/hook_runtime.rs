use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::TraceId;
use systemprompt_security::authz::{
    AllowAllHook, AuthzDecision, AuthzDecisionHook, AuthzRequest, EntityKind,
};
use systemprompt_test_fixtures::fixture_user_id;

#[derive(Debug)]
struct LocalDenyAllHook;

#[async_trait]
impl AuthzDecisionHook for LocalDenyAllHook {
    async fn evaluate(&self, _req: AuthzRequest) -> AuthzDecision {
        AuthzDecision::Deny {
            reason: "test".into(),
            policy: "deny_all_test".into(),
        }
    }
}

fn fixture_request(entity: EntityKind) -> AuthzRequest {
    AuthzRequest {
        entity_type: entity,
        entity_id: "fixture".into(),
        user_id: fixture_user_id(),
        roles: vec!["eng".into()],
        department: "platform".into(),
        trace_id: TraceId::new("trace-test"),
        context: serde_json::Value::Null,
        act_chain: Vec::new(),
    }
}

#[tokio::test]
async fn allow_all_hook_evaluates_to_allow() {
    let hook: Arc<dyn AuthzDecisionHook> = Arc::new(AllowAllHook::null());
    let decision = hook.evaluate(fixture_request(EntityKind::GatewayRoute)).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn deny_all_hook_evaluates_to_deny() {
    let hook: Arc<dyn AuthzDecisionHook> = Arc::new(LocalDenyAllHook);
    let decision = hook.evaluate(fixture_request(EntityKind::McpServer)).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}
