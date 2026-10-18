//! The `rate_limit` policy counted in PostgreSQL: two engines standing in for
//! two replicas share one budget, a re-evaluated call keeps its position, and
//! an engine with no counter store is refused at construction and one whose
//! store is unreachable fails closed.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::json;
use systemprompt_identifiers::{CallId, McpToolName, SessionId, UserId};
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_security::policy::types::{AccessScope, AgentScope};
use systemprompt_security::policy::{
    GovernanceConfig, GovernanceEngine, GovernanceEngineError, PolicyRequest, RateWindowRepository,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};

const LIMIT_3: &str = "governance:\n  policies:\n    - id: rate_limit\n      requests_per_window: 3\n      window_secs: 3600\n";
const LIMIT_3_WARN: &str = "governance:\n  policies:\n    - id: rate_limit\n      mode: warn\n      requests_per_window: 3\n      window_secs: 3600\n";

fn engine_with(yaml: &str, rate_windows: Option<RateWindowRepository>) -> GovernanceEngine {
    GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap(), rate_windows).unwrap()
}

async fn replica(yaml: &str) -> GovernanceEngine {
    ensure_test_bootstrap();
    let db = test_db_pool().await;
    let pool = db.pool();
    engine_with(yaml, Some(RateWindowRepository::new((*pool).clone())))
}

struct Caller {
    session: SessionId,
    user: UserId,
    input: GovernedInput,
}

impl Caller {
    fn new() -> Self {
        Self {
            session: SessionId::generate(),
            user: UserId::new(format!("rate-window-{}", uuid::Uuid::new_v4())),
            input: GovernedInput::tool_arguments(McpToolInput::new(json!({}))),
        }
    }

    async fn call(&self, engine: &GovernanceEngine, call: &CallId) -> Decision {
        engine
            .charge_and_evaluate(PolicyRequest {
                target: GovernedTarget::Tool {
                    tool: McpToolName::try_new("read_file").unwrap(),
                },
                agent_scope: AgentScope::User {
                    user_id: self.user.clone(),
                },
                access_scope: AccessScope::User,
                roles: &[],
                session_id: &self.session,
                user_id: &self.user,
                input: &self.input,
                call_id: call,
            })
            .await
            .decision
    }
}

fn is_rate_denied(decision: &Decision) -> bool {
    matches!(
        decision,
        Decision::Deny {
            reason: DenyReason::RateLimitExceeded { .. }
        }
    )
}

#[tokio::test]
async fn two_replicas_share_one_budget() {
    let a = replica(LIMIT_3).await;
    let b = replica(LIMIT_3).await;
    let caller = Caller::new();

    let mut decisions = Vec::new();
    for engine in [&a, &b, &a, &b] {
        decisions.push(caller.call(engine, &CallId::generate()).await);
    }

    assert!(
        decisions[..3]
            .iter()
            .all(|d| matches!(d, Decision::Allow { .. }))
    );
    assert!(
        is_rate_denied(&decisions[3]),
        "the fourth call across both replicas exceeds the shared limit of 3"
    );
}

#[tokio::test]
async fn a_re_evaluated_call_keeps_its_position() {
    let a = replica(LIMIT_3).await;
    let b = replica(LIMIT_3).await;
    let caller = Caller::new();
    let first = CallId::generate();

    assert!(matches!(
        caller.call(&a, &first).await,
        Decision::Allow { .. }
    ));
    for _ in 0..5 {
        assert!(matches!(
            caller.call(&b, &first).await,
            Decision::Allow { .. }
        ));
    }
    for _ in 0..2 {
        assert!(matches!(
            caller.call(&a, &CallId::generate()).await,
            Decision::Allow { .. }
        ));
    }
    assert!(is_rate_denied(&caller.call(&b, &CallId::generate()).await));
}

#[tokio::test]
async fn sessions_are_budgeted_independently() {
    let a = replica(LIMIT_3).await;
    let first = Caller::new();
    let second = Caller::new();
    for _ in 0..4 {
        caller_noise(&a, &first).await;
    }
    assert!(matches!(
        second.call(&a, &CallId::generate()).await,
        Decision::Allow { .. }
    ));
}

async fn caller_noise(engine: &GovernanceEngine, caller: &Caller) {
    caller.call(engine, &CallId::generate()).await;
}

#[test]
fn an_engine_without_a_counter_store_is_a_construction_error() {
    for yaml in [LIMIT_3, LIMIT_3_WARN] {
        let error = GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap(), None)
            .unwrap_err();
        assert!(
            matches!(&error, GovernanceEngineError::RateWindowsUnavailable { id } if id == "rate_limit"),
            "{error}"
        );
    }
}

#[test]
fn a_chain_without_rate_windows_builds_without_a_counter_store() {
    let yaml = "governance:\n  policies:\n    - id: rate_limit\n      enabled: false\n    - id: tool_blocklist\n";
    GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap(), None)
        .expect("a disabled rate window needs no counter store");
}

#[tokio::test]
async fn an_unreachable_counter_store_fails_closed() {
    let engine = engine_with(LIMIT_3, Some(crate::offline_rate_windows()));
    let caller = Caller::new();
    let decision = caller.call(&engine, &CallId::generate()).await;
    assert!(matches!(
        decision,
        Decision::Deny {
            reason: DenyReason::PolicyViolation { .. }
        }
    ));
}

#[tokio::test]
async fn warn_mode_reports_the_overrun_without_blocking() {
    let a = replica(LIMIT_3_WARN).await;
    let caller = Caller::new();
    for _ in 0..3 {
        caller.call(&a, &CallId::generate()).await;
    }
    assert!(matches!(
        caller.call(&a, &CallId::generate()).await,
        Decision::Warn { .. }
    ));
}
