use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_security::authz::{
    AllowAllHook, AuthzDecision, AuthzDecisionHook, AuthzRequest, EntityKind,
    clear_global_hook, global_hook, install_global_hook,
};

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
        user_id: UserId::new("u-test"),
        roles: vec!["eng".into()],
        department: "platform".into(),
        trace_id: TraceId::new("trace-test"),
        context: serde_json::Value::Null,
    }
}

#[tokio::test]
async fn install_and_replace_hook_round_trip() {
    clear_global_hook();
    assert!(global_hook().is_none(), "no hook installed initially");

    install_global_hook(Arc::new(AllowAllHook::null()));
    let allow_decision = global_hook()
        .expect("AllowAll installed")
        .evaluate(fixture_request(EntityKind::GatewayRoute))
        .await;
    assert_eq!(allow_decision, AuthzDecision::Allow);

    install_global_hook(Arc::new(LocalDenyAllHook));
    let deny_decision = global_hook()
        .expect("DenyAll installed")
        .evaluate(fixture_request(EntityKind::McpServer))
        .await;
    assert!(matches!(deny_decision, AuthzDecision::Deny { .. }));

    clear_global_hook();
    assert!(global_hook().is_none(), "clear_global_hook resets the slot");
}
