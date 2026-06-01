use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_identifiers::{RouteId, TraceId, UserId};
use systemprompt_security::authz::{
    AllowAllHook, AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzRequest, AuthzSource,
    CompositeAuthzHook, DenyAllHook, DenyReason, EntityRef,
};

fn make_req() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("test-route")),
        user_id: UserId::new("user-1"),
        roles: vec!["user".to_owned()],
        attributes: BTreeMap::new(),
        trace_id: TraceId::new("trace-1"),
        session_id: None,
        context: AuthzContext::none(),
        act_chain: vec![],
    }
}

#[tokio::test]
async fn deny_all_hook_returns_deny() {
    let hook = DenyAllHook::null();
    let req = make_req();
    let decision = hook.evaluate(req).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}

#[tokio::test]
async fn deny_all_hook_deny_reason_is_hook_unavailable() {
    let hook = DenyAllHook::null();
    let req = make_req();
    let decision = hook.evaluate(req).await;
    if let AuthzDecision::Deny { reason, .. } = decision {
        assert!(
            matches!(reason, DenyReason::HookUnavailable { .. }),
            "got: {reason:?}"
        );
    } else {
        panic!("expected Deny");
    }
}

#[tokio::test]
async fn deny_all_hook_policy_is_deny_all_default() {
    let hook = DenyAllHook::null();
    let req = make_req();
    let decision = hook.evaluate(req).await;
    if let AuthzDecision::Deny { policy, .. } = decision {
        assert_eq!(policy, AuthzSource::DenyAllDefault.policy());
    } else {
        panic!("expected Deny");
    }
}

#[tokio::test]
async fn allow_all_hook_returns_allow() {
    let hook = AllowAllHook::null();
    let req = make_req();
    let decision = hook.evaluate(req).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn composite_empty_hooks_returns_allow() {
    let composite = CompositeAuthzHook::new(vec![]);
    let req = make_req();
    let decision = composite.evaluate(req).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn composite_all_allow_returns_allow() {
    let composite = CompositeAuthzHook::new(vec![
        Arc::new(AllowAllHook::null()),
        Arc::new(AllowAllHook::null()),
    ]);
    let req = make_req();
    let decision = composite.evaluate(req).await;
    assert_eq!(decision, AuthzDecision::Allow);
}

#[tokio::test]
async fn composite_deny_short_circuits() {
    let composite = CompositeAuthzHook::new(vec![
        Arc::new(AllowAllHook::null()),
        Arc::new(DenyAllHook::null()),
        Arc::new(AllowAllHook::null()),
    ]);
    let req = make_req();
    let decision = composite.evaluate(req).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}

#[tokio::test]
async fn composite_first_deny_wins() {
    let composite = CompositeAuthzHook::new(vec![
        Arc::new(DenyAllHook::null()),
        Arc::new(AllowAllHook::null()),
    ]);
    let req = make_req();
    let decision = composite.evaluate(req).await;
    assert!(matches!(decision, AuthzDecision::Deny { .. }));
}

#[test]
fn authz_source_policy_strings() {
    assert_eq!(AuthzSource::WebhookFault.policy(), "authz_hook_fault");
    assert_eq!(AuthzSource::DenyAllDefault.policy(), "authz_default_deny");
    assert_eq!(
        AuthzSource::AllowAllUnrestricted.policy(),
        "authz_unrestricted"
    );
    assert_eq!(AuthzSource::ExtensionHook.policy(), "authz_extension_hook");
    assert_eq!(AuthzSource::RuleBased.policy(), "authz_rule_based");
}

#[test]
fn deny_all_hook_debug_format() {
    let hook = DenyAllHook::null();
    let s = format!("{hook:?}");
    assert!(s.contains("DenyAllHook"), "got: {s}");
}

#[test]
fn allow_all_hook_debug_format() {
    let hook = AllowAllHook::null();
    let s = format!("{hook:?}");
    assert!(s.contains("AllowAllHook"), "got: {s}");
}
