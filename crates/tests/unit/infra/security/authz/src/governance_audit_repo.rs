//! DB-backed behavioural tests for [`GovernanceDecisionRepository`].
//!
//! The repository is the single writer for `governance_decisions`. These tests
//! assert both halves of its contract: a healthy pool persists an audit row
//! that reads back with the values written, and a dead pool propagates the
//! typed `sqlx::Error` rather than silently swallowing it.

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_security::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
    list_trace_ids_with_decision,
};
use systemprompt_test_fixtures::{
    DisposableDb, closed_db_pool, fixture_database_url, fixture_db_pool, seed_user_row,
};
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
        context_id: "ctx_unit_test",
        task_id: None,
        trace_id: None,
        client_id: None,
        tool_use_id: None,
    }
}

fn error_subscriber_guard() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .with_test_writer()
        .finish();
    tracing::subscriber::set_default(subscriber)
}

#[tokio::test]
async fn insert_through_closed_pool_propagates_sqlx_error() {
    // An ERROR-level subscriber is active so the audit-drop log fields (actor
    // id, session id, policy, decision) are evaluated on the failure path.
    let _guard = error_subscriber_guard();
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
        matches!(
            err,
            systemprompt_database::RepositoryError::Database(sqlx::Error::PoolClosed)
        ),
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

#[tokio::test]
async fn trace_lookup_is_distinct_and_filters_by_decision_and_time()
-> Result<(), Box<dyn std::error::Error>> {
    let owned = DisposableDb::installed("governance_trace_lookup").await?;
    let db = owned.pool().await?;
    let pool = db.write_pool_arc().expect("write pool");
    let actor = Actor::user(UserId::new("audit-trace-user"));
    seed_user_row(&db, &actor.user_id, "audit-trace-user@example.invalid").await?;
    let evaluated = serde_json::json!([]);
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);

    for (id, decision, trace_id) in [
        ("recent-deny-a", DecisionTag::Deny, Some("trace-recent")),
        ("recent-deny-b", DecisionTag::Deny, Some("trace-recent")),
        ("recent-allow", DecisionTag::Allow, Some("trace-allow")),
        ("null-deny", DecisionTag::Deny, None),
    ] {
        let mut row = record(id, &actor, &evaluated);
        row.decision = decision;
        row.trace_id = trace_id;
        GovernanceDecisionRepository::from_pool(pool.clone())
            .insert(&row)
            .await?;
    }
    sqlx::query(
        "INSERT INTO governance_decisions(\
         id,user_id,session_id,tool_name,decision,policy,reason,evaluated_rules,\
         actor_kind,actor_id,act_chain,context_id,trace_id,created_at) \
         VALUES($1,$2,'sess-audit','audit-tool','deny','authz_default_deny','unit-test',\
         '[]'::jsonb,'user',$2,'[]'::jsonb,'ctx_unit_test','trace-old',$3)",
    )
    .bind("old-deny")
    .bind(actor.user_id.as_str())
    .bind(cutoff - chrono::Duration::hours(1))
    .execute(pool.as_ref())
    .await?;

    let mut all = list_trace_ids_with_decision(pool.as_ref(), "deny", None).await?;
    all.sort();
    assert_eq!(all, vec!["trace-old".to_owned(), "trace-recent".to_owned()]);
    let recent = list_trace_ids_with_decision(pool.as_ref(), "deny", Some(cutoff)).await?;
    assert_eq!(recent, vec!["trace-recent".to_owned()]);
    let allowed = list_trace_ids_with_decision(pool.as_ref(), "allow", None).await?;
    assert_eq!(allowed, vec!["trace-allow".to_owned()]);

    pool.close().await;
    drop(pool);
    drop(db);
    owned.drop_now().await;
    Ok(())
}
