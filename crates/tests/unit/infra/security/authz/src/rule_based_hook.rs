//! Fail-closed contract for [`RuleBasedHook`].
//!
//! When the backing pool cannot answer (the entity/rule lookup errors), the
//! hook must deny under the `authz_rule_based` policy rather than fall open.

use std::sync::Arc;

use systemprompt_governance::authz::types::{Access, EntityKind, RuleType};
use systemprompt_governance::authz::{
    AccessControlRepository, AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzRequest,
    ChainSources, DenyReason, EntityRef, NullAuditSink, RuleBasedHook, UpsertRuleParams,
};
use systemprompt_identifiers::{RouteId, TraceId};
use systemprompt_test_fixtures::closed_db_pool;

fn fixture() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: systemprompt_test_fixtures::fixture_user_id(),
        actor: None,
        client_id: None,
        access_scope: None,
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
    let pool = db.write_pool();
    let hook = RuleBasedHook::new(
        pool,
        Arc::new(NullAuditSink),
        systemprompt_governance::authz::AuthzWiring {
            sources: ChainSources::default(),
            epochs: systemprompt_traits::CacheEpochs::new(),
            services: systemprompt_test_fixtures::fixture_services_view(),
            subjects: Default::default(),
            providers: Default::default(),
        },
    );

    let decision = hook.evaluate(fixture()).await;
    match decision {
        AuthzDecision::Deny { policy, reason } => {
            assert_eq!(policy, "authz_rule_based");
            assert!(matches!(
                reason,
                DenyReason::HookUnavailable { policy: p, .. } if p == "authz_rule_based"
            ));
        },
        AuthzDecision::Allow => panic!("a dead pool must fail closed, got Allow"),
    }
}

fn request_for(route: &str, user: &systemprompt_identifiers::UserId) -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new(route)),
        user_id: user.clone(),
        ..fixture()
    }
}

#[tokio::test]
async fn a_rule_added_after_a_cached_decision_applies_once_the_index_revalidates() {
    let db = systemprompt_test_fixtures::test_db_pool().await;
    let pool = db.write_pool();
    let repo = AccessControlRepository::from_pool(std::sync::Arc::clone(&pool));
    let route = format!("rb-cache-{}", uuid::Uuid::new_v4().simple());
    let user = systemprompt_test_fixtures::unique_user_id("rb-cache");
    repo.upsert_entity(EntityKind::GatewayRoute, &route, false, "code:test")
        .await
        .expect("entity");
    let hook = RuleBasedHook::new(
        pool,
        Arc::new(NullAuditSink),
        systemprompt_governance::authz::AuthzWiring {
            sources: ChainSources::default(),
            epochs: systemprompt_traits::CacheEpochs::new(),
            services: systemprompt_test_fixtures::fixture_services_view(),
            subjects: Default::default(),
            providers: Default::default(),
        },
    );

    for _ in 0..3 {
        assert!(matches!(
            hook.evaluate(request_for(&route, &user)).await,
            AuthzDecision::Deny { .. }
        ));
    }

    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::GatewayRoute,
        entity_id: &route,
        rule_type: RuleType::USER,
        rule_value: user.as_str(),
        access: Access::Allow,
        justification: None,
        source: "code:test",
    })
    .await
    .expect("rule");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut decision = hook.evaluate(request_for(&route, &user)).await;
    while !matches!(decision, AuthzDecision::Allow) && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        decision = hook.evaluate(request_for(&route, &user)).await;
    }
    assert!(matches!(decision, AuthzDecision::Allow));
}
