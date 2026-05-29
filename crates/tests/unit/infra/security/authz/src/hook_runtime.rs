use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{McpServerId, RouteId, TraceId};
use systemprompt_security::authz::{
    AllowAllHook, AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzRequest, DenyReason,
    EntityRef,
};
use systemprompt_test_fixtures::fixture_user_id;

#[derive(Debug)]
struct LocalDenyAllHook;

#[async_trait]
impl AuthzDecisionHook for LocalDenyAllHook {
    async fn evaluate(&self, _req: AuthzRequest) -> AuthzDecision {
        AuthzDecision::Deny {
            reason: DenyReason::HookUnavailable {
                policy: "deny_all_test".into(),
            },
            policy: "deny_all_test".into(),
        }
    }
}

fn fixture_request(entity: EntityRef) -> AuthzRequest {
    AuthzRequest {
        entity,
        user_id: fixture_user_id(),
        roles: vec!["eng".into()],
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new("trace-test"),
        session_id: None,
        context: AuthzContext::none(),
        act_chain: Vec::new(),
    }
}

#[tokio::test]
async fn allow_all_hook_evaluates_to_allow() {
    let hook: Arc<dyn AuthzDecisionHook> = Arc::new(AllowAllHook::null());
    let decision = hook
        .evaluate(fixture_request(EntityRef::GatewayRoute(RouteId::new(
            "fixture",
        ))))
        .await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn deny_all_hook_evaluates_to_deny() {
    let hook: Arc<dyn AuthzDecisionHook> = Arc::new(LocalDenyAllHook);
    let decision = hook
        .evaluate(fixture_request(EntityRef::McpServer(McpServerId::new(
            "fixture",
        ))))
        .await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}
