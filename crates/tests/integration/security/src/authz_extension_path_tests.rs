//! End-to-end coverage for the extension authz path: profile +
//! [`build_authz_hook`] + supplied hook + `DbAuditSink` round-trip.
//!
//! Verifies that when `governance.authz.hook.mode = extension` and the
//! caller supplies a hook, evaluating the composed pipeline (a) lets the
//! extension hook fire after the core RuleBasedHook allows the entity and
//! (b) lands an audit row tagged with [`AuthzSource::ExtensionHook`].

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_identifiers::{RouteId, TraceId};
use systemprompt_models::profile::{AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig};
use systemprompt_security::authz::{
    AuthzAuditSink, AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzRequest, AuthzSource,
    EntityRef, SharedAuthzHook, build_authz_hook,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

fn extension_governance() -> GovernanceConfig {
    GovernanceConfig {
        authz: Some(AuthzConfig {
            hook: AuthzHookConfig {
                mode: AuthzMode::Extension,
                url: None,
                timeout_ms: 100,
                acknowledgement: None,
            },
        }),
    }
}

fn request_with_trace(route: &str, trace: &str) -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new(route)),
        user_id: fixture_user_id(),
        roles: vec!["eng".into()],
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new(trace),
        session_id: None,
        context: AuthzContext::none(),
        act_chain: Vec::new(),
    }
}

#[derive(Debug)]
struct RecordingHook {
    calls: Arc<Mutex<Vec<TraceId>>>,
    sink: Arc<dyn AuthzAuditSink>,
}

#[async_trait]
impl AuthzDecisionHook for RecordingHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        self.calls.lock().unwrap().push(req.trace_id.clone());
        let decision = AuthzDecision::Allow;
        self.sink
            .record(&req, &decision, AuthzSource::ExtensionHook)
            .await;
        decision
    }
}

#[tokio::test]
async fn extension_hook_evaluated_and_audited() {
    let url = match fixture_database_url() {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping: DATABASE_URL unset");
            return;
        },
    };
    let pool = fixture_db_pool(&url).await.expect("connect db");
    let write_pool = pool.write_pool_arc().expect("write pool");

    let route = format!("route-{}", uuid::Uuid::new_v4());
    // Why: the composite now runs RuleBasedHook ahead of the extension hook;
    // it would deny unknown entities. Register a default-included catalog row
    // so the extension hook receives the request.
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) \
         VALUES ($1, $2, true, $3) ON CONFLICT (entity_type, entity_id) DO NOTHING",
    )
    .bind("gateway_route")
    .bind(&route)
    .bind("test:extension_hook")
    .execute(write_pool.as_ref())
    .await
    .expect("seed entity row");

    let sink: Arc<dyn AuthzAuditSink> = Arc::new(systemprompt_security::authz::DbAuditSink::new(
        systemprompt_security::authz::GovernanceDecisionRepository::from_pool(write_pool.clone()),
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let hook: SharedAuthzHook = Arc::new(RecordingHook {
        calls: calls.clone(),
        sink,
    });

    let governance = extension_governance();
    let built = build_authz_hook(Some(&governance), Some(write_pool.clone()), Some(hook))
        .expect("bootstrap ok");

    let trace = format!("trace-{}", uuid::Uuid::new_v4());
    let decision = built.evaluate(request_with_trace(&route, &trace)).await;
    assert_eq!(decision, AuthzDecision::Allow);

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1, "hook should be evaluated exactly once");
    assert_eq!(recorded[0].as_str(), trace);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM governance_decisions WHERE session_id = $1 AND policy = $2",
    )
    .bind(&trace)
    .bind(AuthzSource::ExtensionHook.policy())
    .fetch_one(write_pool.as_ref())
    .await
    .expect("query audit rows");
    assert_eq!(count, 1, "DbAuditSink should have written one audit row");
}
