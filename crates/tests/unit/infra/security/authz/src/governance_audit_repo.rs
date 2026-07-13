//! DB-backed behavioural tests for [`GovernanceDecisionRepository`].
//!
//! The repository is the single writer for `governance_decisions`. These tests
//! assert both halves of its contract: a healthy pool persists an audit row
//! that reads back with the values written, and a dead pool propagates the
//! typed `sqlx::Error` rather than silently swallowing it.

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_security::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_test_fixtures::{closed_db_pool, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn record<'a>(
    id: &'a str,
    actor: &'a Actor,
    evaluated: &'a serde_json::Value,
) -> GovernanceDecisionRecord<'a> {
    GovernanceDecisionRecord {
        id,
        actor,
        session_id: "sess-audit",
        tool_name: "audit-tool",
        agent_id: None,
        agent_scope: None,
        decision: DecisionTag::Deny,
        policy: "authz_default_deny",
        reason: "unit-test",
        evaluated_rules: evaluated,
        plugin_id: None,
        act_chain: &[],
        context_id: None,
        task_id: None,
    }
}

#[tokio::test]
async fn insert_through_closed_pool_propagates_sqlx_error() {
    let db = closed_db_pool().await;
    let pool = db
        .write_pool_arc()
        .expect("closed pool still exposes a write handle");
    let repo = GovernanceDecisionRepository::from_pool(pool);

    let id = Uuid::new_v4().to_string();
    let actor = Actor::user(UserId::new("audit-user"));
    let evaluated = serde_json::json!([]);
    let err = repo
        .insert(&record(&id, &actor, &evaluated))
        .await
        .expect_err("a closed pool must surface the failure, not drop the audit row");
    assert!(
        matches!(err, sqlx::Error::PoolClosed),
        "expected PoolClosed, got {err:?}"
    );
}

#[tokio::test]
async fn insert_persists_a_decision_row() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let pool = db.write_pool_arc().expect("write pool");
    let repo = GovernanceDecisionRepository::from_pool(pool.clone());

    // pool() exposes the same handle the repository writes through.
    assert!(!repo.pool().is_closed(), "live repository pool is open");

    let id = Uuid::new_v4().to_string();
    let actor = Actor::user(UserId::new("audit-user-live"));
    let evaluated = serde_json::json!({"source": "unit-test"});
    repo.insert(&record(&id, &actor, &evaluated))
        .await
        .expect("insert succeeds against a live pool");

    let row: (String, String, String, String) = sqlx::query_as(
        "SELECT policy, decision, reason, actor_kind FROM governance_decisions WHERE id = $1",
    )
    .bind(&id)
    .fetch_one(&*pool)
    .await
    .expect("row is queryable after insert");
    assert_eq!(row.0, "authz_default_deny");
    assert_eq!(row.1, "deny");
    assert_eq!(row.2, "unit-test");
    assert_eq!(row.3, "user");

    sqlx::query("DELETE FROM governance_decisions WHERE id = $1")
        .bind(&id)
        .execute(&*pool)
        .await
        .expect("cleanup");
}
