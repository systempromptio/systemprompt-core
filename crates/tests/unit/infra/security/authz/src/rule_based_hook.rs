//! Fail-closed contract for [`RuleBasedHook`].
//!
//! When the backing pool cannot answer (the entity/rule lookup errors), the
//! hook must deny under the `authz_rule_based` policy rather than fall open.

use std::sync::Arc;

use systemprompt_identifiers::{RouteId, TraceId};
use systemprompt_security::authz::{
    AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzRequest, DenyReason, EntityRef,
    NullAuditSink, RuleBasedHook,
};
use systemprompt_test_fixtures::closed_db_pool;

fn fixture() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: systemprompt_test_fixtures::fixture_user_id(),
        roles: vec!["eng".into()],
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new("trace-rb"),
        session_id: None,
        context: AuthzContext::none(),
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}

#[tokio::test]
async fn rule_based_hook_denies_when_the_pool_is_unavailable() {
    let db = closed_db_pool().await;
    let pool = db.write_pool_arc().expect("closed pool still exposes a write handle");
    let hook = RuleBasedHook::new(pool, Arc::new(NullAuditSink));

    let decision = hook.evaluate(fixture()).await;
    match decision {
        AuthzDecision::Deny { policy, reason } => {
            assert_eq!(policy, "authz_rule_based");
            assert!(matches!(
                reason,
                DenyReason::HookUnavailable { policy: p } if p == "authz_rule_based"
            ));
        },
        AuthzDecision::Allow => panic!("a dead pool must fail closed, got Allow"),
    }
}
