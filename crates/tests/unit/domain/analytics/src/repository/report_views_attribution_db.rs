//! The attribution, tool-call and governance-decision report views: an
//! extension reading them sees a request's scopes, its tool calls and the
//! decisions on its session, and nothing of a deleted user.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use uuid::Uuid;

async fn seed(raw: &sqlx::PgPool, user: &UserId, tag: &str) -> String {
    let request = format!("req-{tag}");
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, \
         actor_kind, actor_id, status) \
         VALUES ($1, $2, $3, '3f2a1c4e-8b7d-4c2a-9e1f-0a1b2c3d4e5f', 'anthropic', 'm', \
         'user', $3, 'completed')",
    )
    .bind(&request)
    .bind(format!("wire-{tag}"))
    .bind(user.as_str())
    .execute(raw)
    .await
    .expect("insert request");
    sqlx::query(
        "INSERT INTO ai_request_attributions (request_id, dimension, value, source) \
         VALUES ($1, 'harness_generation', 'g42', 'header'), ($1, 'ring', '1', 'header')",
    )
    .bind(&request)
    .execute(raw)
    .await
    .expect("insert attributions");
    sqlx::query(
        "INSERT INTO ai_request_tool_calls (request_id, tool_name, tool_input, sequence_number) \
         VALUES ($1, 'bash', '{\"cmd\":\"secret\"}', 0)",
    )
    .bind(&request)
    .execute(raw)
    .await
    .expect("insert tool call");
    sqlx::query(
        "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, decision, policy, \
         reason, actor_kind, actor_id, context_id) \
         VALUES ($1, $2, $3, 'bash', 'deny', 'shell', 'denied', 'user', $2, 'ctx')",
    )
    .bind(format!("decision-{tag}"))
    .bind(user.as_str())
    .bind(format!("session-{tag}"))
    .execute(raw)
    .await
    .expect("insert decision");
    request
}

async fn count(raw: &sqlx::PgPool, sql: &'static str, key: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(key)
        .fetch_one(raw)
        .await
        .expect("count report rows")
}

#[tokio::test]
async fn attribution_reports_show_live_users_and_hide_deleted_ones() {
    let database = DisposableDb::with_schema("analytics_report_attribution").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let suffix = Uuid::new_v4().simple().to_string();
    let live = UserId::new(format!("live-{suffix}"));
    let gone = UserId::new(format!("gone-{suffix}"));
    for user in [&live, &gone] {
        seed_user_row(&pool, user, &format!("{}@report.invalid", user.as_str()))
            .await
            .expect("seed user");
    }
    let live_request = seed(raw.as_ref(), &live, &format!("live-{suffix}")).await;
    let gone_request = seed(raw.as_ref(), &gone, &format!("gone-{suffix}")).await;
    sqlx::query("UPDATE users SET status = 'deleted' WHERE id = $1")
        .bind(gone.as_str())
        .execute(raw.as_ref())
        .await
        .expect("delete user");

    let attributions = "SELECT COUNT(*) FROM report_ai_request_attributions WHERE request_id = $1";
    let tool_calls = "SELECT COUNT(*) FROM report_ai_request_tool_calls WHERE request_id = $1";
    let decisions = "SELECT COUNT(*) FROM report_governance_decisions WHERE user_id = $1";

    assert_eq!(count(raw.as_ref(), attributions, &live_request).await, 2);
    assert_eq!(count(raw.as_ref(), tool_calls, &live_request).await, 1);
    assert_eq!(count(raw.as_ref(), decisions, live.as_str()).await, 1);
    assert_eq!(count(raw.as_ref(), attributions, &gone_request).await, 0);
    assert_eq!(count(raw.as_ref(), tool_calls, &gone_request).await, 0);
    assert_eq!(count(raw.as_ref(), decisions, gone.as_str()).await, 0);

    let exposes_input: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
         WHERE table_name = 'report_ai_request_tool_calls' \
         AND column_name IN ('tool_input', 'tool_result_payload'))",
    )
    .fetch_one(raw.as_ref())
    .await
    .expect("read view columns");
    assert!(
        !exposes_input,
        "tool arguments and results stay in the source table"
    );
}
