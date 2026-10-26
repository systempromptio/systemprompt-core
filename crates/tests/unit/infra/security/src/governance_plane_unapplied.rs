//! An empty, never-applied `governance` plane follows the profile's
//! `unapplied_planes`: the default `warn` passes every call as a `Warn`
//! carrying the apply hint, `deny` refuses it with that hint, and a freshly
//! applied chain enforces whatever the mode.

use serde_json::json;
use systemprompt_database::DbPool;
use systemprompt_governance::authz::types::{Decision, DenyReason};
use systemprompt_governance::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_governance::policy::types::{AccessScope, AgentScope, PolicyContext};
use systemprompt_governance::policy::{GovernanceEngine, RateCounts};
use systemprompt_identifiers::{CallId, McpToolName, SessionId, UserId};
use systemprompt_manifest::profile::UnappliedPlaneMode;
use systemprompt_test_fixtures::{DisposableDb, apply_governance_plane};

const ENFORCED_SECRET_SCAN: &str = "governance:
  mode: enforce
  policies:
    - id: secret_scan
      patterns:
        - id: cloud-access-key
          name: Cloud Access Key
          regex: 'AKIA[0-9A-Z]{16}'
          redact_whole_value: true
";

static NO_RATE_COUNTS: RateCounts = RateCounts::empty();

async fn load(pool: &DbPool, mode: UnappliedPlaneMode) -> GovernanceEngine {
    GovernanceEngine::load(
        pool.pool().as_ref(),
        Some(crate::offline_rate_windows()),
        mode,
    )
    .await
    .expect("engine loads")
}

fn not_applied() -> DenyReason {
    DenyReason::PlaneNotApplied {
        plane: "governance".to_owned(),
    }
}

fn decide(engine: &GovernanceEngine, arguments: serde_json::Value) -> Decision {
    let session = SessionId::generate();
    let user = UserId::new("plane-user");
    let call = CallId::generate();
    let input = GovernedInput::tool_arguments(McpToolInput::new(arguments));
    let ctx = PolicyContext {
        target: GovernedTarget::Tool {
            tool: McpToolName::try_new("upload").expect("valid McpToolName"),
        },
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::User,
        roles: &[],
        session_id: &session,
        user_id: &user,
        input: &input,
        call_id: &call,
        rate_counts: &NO_RATE_COUNTS,
    };
    engine.evaluate(&ctx).decision
}

#[tokio::test]
async fn the_default_mode_passes_an_unapplied_plane_with_a_warning() {
    let db = DisposableDb::with_schema("govunappliedwarn").await;
    let pool = db.test_pool().await;

    let engine = load(&pool, UnappliedPlaneMode::default()).await;
    assert_eq!(engine.unapplied_mode(), Some(UnappliedPlaneMode::Warn));
    let decision = decide(&engine, json!({"key": "AKIAIOSFODNN7EXAMPLE"}));
    assert!(decision.permits(), "warn mode lets the call through");
    assert_eq!(
        decision,
        Decision::Warn {
            reason: not_applied()
        }
    );
    db.drop_now().await;
}

#[tokio::test]
async fn deny_mode_refuses_an_unapplied_plane_with_the_apply_hint() {
    let db = DisposableDb::with_schema("govunapplieddeny").await;
    let pool = db.test_pool().await;

    let engine = load(&pool, UnappliedPlaneMode::Deny).await;
    assert!(!engine.is_applied());
    assert!(!engine.enforces_prompt_secrets());
    let Decision::Deny { reason } = decide(&engine, json!({"key": "harmless"})) else {
        panic!("deny mode must refuse");
    };
    assert_eq!(reason, not_applied());
    assert_eq!(
        reason.to_string(),
        "governance plane not applied — run `systemprompt admin config apply`"
    );
    db.drop_now().await;
}

#[tokio::test]
async fn a_freshly_applied_plane_enforces_its_chain() {
    let db = DisposableDb::with_schema("govapplied").await;
    let pool = db.test_pool().await;
    apply_governance_plane(&pool, ENFORCED_SECRET_SCAN)
        .await
        .expect("apply");

    let engine = load(&pool, UnappliedPlaneMode::Deny).await;
    assert!(engine.is_applied());
    assert!(decide(&engine, json!({"key": "harmless"})).permits());
    assert!(matches!(
        decide(&engine, json!({"key": "AKIAIOSFODNN7EXAMPLE"})),
        Decision::Deny {
            reason: DenyReason::SecretLeak { .. }
        }
    ));
    db.drop_now().await;
}
