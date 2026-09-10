//! The `governance_decisions` append-only guarantee, exercised against a live
//! database.
//!
//! Immutability used to be a convention the operator was expected to install as
//! a grant. A migration replaces that with a BEFORE UPDATE trigger, which
//! binds every role including the one these tests connect as — so the assertion
//! here is that an UPDATE is refused while INSERT and DELETE still work.

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_security::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn record<'a>(
    id: &'a str,
    actor: &'a Actor,
    evaluated: &'a serde_json::Value,
) -> GovernanceDecisionRecord<'a> {
    GovernanceDecisionRecord {
        id,
        actor,
        session_id: "sess-append-only",
        tool_name: "append-only-tool",
        agent_id: None,
        agent_scope: None,
        decision: DecisionTag::Allow,
        policy: "authz_default_allow",
        reason: "append-only test",
        evaluated_rules: evaluated,
        plugin_id: None,
        act_chain: &[],
        context_id: "ctx_append_only",
        task_id: None,
        trace_id: None,
        client_id: None,
    }
}

#[tokio::test]
async fn a_recorded_decision_cannot_be_rewritten() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let pool = db.write_pool_arc().expect("write pool");
    let repo = GovernanceDecisionRepository::from_pool(pool.clone());

    let id = Uuid::new_v4().to_string();
    let actor = Actor::user(UserId::new("append-only-user"));
    let evaluated = serde_json::json!({"source": "append-only-test"});
    repo.insert(&record(&id, &actor, &evaluated))
        .await
        .expect("insert succeeds");

    let err = sqlx::query("UPDATE governance_decisions SET user_id = $1 WHERE id = $2")
        .bind("someone-else")
        .bind(&id)
        .execute(&*pool)
        .await
        .expect_err("an UPDATE against the audit trail must be refused");
    let message = err.to_string();
    assert!(
        message.contains("append-only"),
        "the refusal must name the reason, got: {message}"
    );

    let user_id: (String,) =
        sqlx::query_as("SELECT user_id FROM governance_decisions WHERE id = $1")
            .bind(&id)
            .fetch_one(&*pool)
            .await
            .expect("the row survives the refused update");
    assert_eq!(user_id.0, "append-only-user");

    let deleted = sqlx::query("DELETE FROM governance_decisions WHERE id = $1")
        .bind(&id)
        .execute(&*pool)
        .await
        .expect("DELETE stays available for retention and erasure");
    assert_eq!(deleted.rows_affected(), 1);
}
