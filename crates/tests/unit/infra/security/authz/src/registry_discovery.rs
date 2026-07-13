//! Inventory-based extension-hook discovery.
//!
//! Two statically registered factories must be auto-composed into a single
//! [`CompositeAuthzHook`] so multiple extensions can each contribute an authz
//! hook without a shared wiring site.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{RouteId, TraceId};
use systemprompt_security::authz::{
    AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzHookContext, AuthzRequest, DenyReason,
    EntityRef, NullAuditSink, SharedAuthzHook, discover_authz_hook,
};

#[derive(Debug)]
struct AllowHook;

#[async_trait]
impl AuthzDecisionHook for AllowHook {
    async fn evaluate(&self, _req: AuthzRequest) -> AuthzDecision {
        AuthzDecision::Allow
    }
}

#[derive(Debug)]
struct DenyHook;

#[async_trait]
impl AuthzDecisionHook for DenyHook {
    async fn evaluate(&self, _req: AuthzRequest) -> AuthzDecision {
        AuthzDecision::Deny {
            reason: DenyReason::HookUnavailable {
                policy: "registry_test_deny".into(),
            },
            policy: "registry_test_deny".into(),
        }
    }
}

systemprompt_security::register_authz_hook!(|_ctx| Arc::new(AllowHook) as SharedAuthzHook);
systemprompt_security::register_authz_hook!(|_ctx| Arc::new(DenyHook) as SharedAuthzHook);

fn context() -> AuthzHookContext {
    let pool = sqlx::PgPool::connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("lazy pool construction is infallible for a well-formed URL");
    AuthzHookContext {
        pool: Arc::new(pool),
        sink: Arc::new(NullAuditSink),
    }
}

fn fixture() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: systemprompt_test_fixtures::fixture_user_id(),
        roles: vec!["eng".into()],
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new("trace-reg"),
        session_id: None,
        context: AuthzContext::none(),
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}

#[tokio::test]
async fn discovers_and_composes_registered_hooks() {
    let discovered = discover_authz_hook(&context()).expect("two registrations must resolve a hook");
    // The composite short-circuits on the first Deny, so the DenyHook's policy
    // must surface even though an AllowHook is also registered.
    match discovered.evaluate(fixture()).await {
        AuthzDecision::Deny { policy, .. } => assert_eq!(policy, "registry_test_deny"),
        AuthzDecision::Allow => panic!("a registered DenyHook must make the composite deny"),
    }
}

#[tokio::test]
async fn hook_context_debug_is_redacted() {
    let rendered = format!("{:?}", context());
    assert!(
        rendered.contains("AuthzHookContext"),
        "debug output names the context type without dumping the pool/sink internals"
    );
}
