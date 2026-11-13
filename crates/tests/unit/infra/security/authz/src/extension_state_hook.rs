//! An authz hook factory reaches extension state through
//! `AuthzHookContext::state`: the factory runs before the state exists and
//! captures the handle, the host binds it once, and the hook then reads the
//! very instance the extension provided. Before binding the hook fails
//! closed.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::StateHandle;
use systemprompt_extension::prelude::Dependencies;
use systemprompt_governance::authz::{
    AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzHookContext, AuthzRequest, DenyReason,
    EntityRef, NullAuditSink, SharedAuthzHook,
};
use systemprompt_identifiers::{ExtensionId, RouteId, TraceId};

#[derive(Debug)]
struct GrantCache {
    allow: bool,
}

#[derive(Debug)]
struct CacheHook {
    state: StateHandle,
}

#[async_trait]
impl AuthzDecisionHook for CacheHook {
    async fn evaluate(&self, _req: AuthzRequest) -> AuthzDecision {
        match self.state.get_arc::<GrantCache>() {
            Ok(cache) if cache.allow => AuthzDecision::Allow,
            _ => AuthzDecision::Deny {
                reason: DenyReason::HookUnavailable {
                    policy: "grant_cache".into(),
                    detail: String::new(),
                },
                policy: "grant_cache".into(),
            },
        }
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        self.evaluate(req).await
    }
}

fn cache_hook_factory(ctx: &AuthzHookContext) -> SharedAuthzHook {
    Arc::new(CacheHook {
        state: ctx.state.clone(),
    })
}

fn context(state: StateHandle) -> AuthzHookContext {
    let pool = sqlx::PgPool::connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("lazy pool construction is infallible for a well-formed URL");
    AuthzHookContext {
        pool: Arc::new(pool),
        sink: Arc::new(NullAuditSink),
        services: systemprompt_test_fixtures::fixture_services_view(),
        subjects: Default::default(),
        secrets: systemprompt_test_fixtures::fixture_secret_store(),
        state,
    }
}

fn request() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: systemprompt_test_fixtures::fixture_user_id(),
        actor: None,
        client_id: None,
        access_scope: None,
        roles: vec![],
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new("trace-state"),
        session_id: None,
        context: AuthzContext::none(),
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}

#[tokio::test]
async fn the_hook_reads_the_instance_the_extension_provided_once_bound() {
    let handle = StateHandle::default();
    let hook = cache_hook_factory(&context(handle.clone()));

    assert!(
        matches!(hook.evaluate(request()).await, AuthzDecision::Deny { .. }),
        "an unbound state fails closed"
    );

    let provided = Arc::new(GrantCache { allow: true });
    let mut state = Dependencies::new();
    state
        .provide(&ExtensionId::new("grants"), Arc::clone(&provided))
        .expect("first provider");
    handle.bind(state.clone()).expect("bound once");

    assert!(matches!(
        hook.evaluate(request()).await,
        AuthzDecision::Allow
    ));
    let read_by_hook = handle.get_arc::<GrantCache>().expect("bound state");
    let read_by_router = state.get_arc::<GrantCache>().expect("same state");
    assert!(Arc::ptr_eq(&read_by_hook, &provided));
    assert!(Arc::ptr_eq(&read_by_router, &provided));
}

#[test]
fn a_handle_binds_only_once() {
    let handle = StateHandle::default();
    handle.bind(Dependencies::new()).expect("first bind");
    assert!(matches!(
        handle.bind(Dependencies::new()),
        Err(systemprompt_extension::ExtensionStateError::AlreadyBound)
    ));
}

#[test]
fn an_unbound_handle_reports_not_ready() {
    assert!(matches!(
        StateHandle::default().get_arc::<GrantCache>(),
        Err(systemprompt_extension::ExtensionStateError::NotReady)
    ));
}
