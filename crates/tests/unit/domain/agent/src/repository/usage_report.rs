use super::{repos, seed_context_and_task, seed_user_and_session};
use chrono::{Duration, Utc};
use systemprompt_agent::repository::AgentActivityReportRepository;
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::usage_reports::agent::AgentActivityReport;

#[tokio::test]
async fn agent_contexts_are_listed_counted_and_recognised_as_sessions() {
    let pool = test_db_pool().await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let start = Utc::now() - Duration::minutes(1);
    let (context_id, _task_id) = seed_context_and_task(&repos(&pool), &user_id, &session_id).await;
    let end = Utc::now() + Duration::minutes(1);
    let report = AgentActivityReportRepository::new(&pool);

    let contexts = report
        .list_agent_contexts(start, end, 50, Some(&user_id))
        .await
        .expect("list agent contexts");
    let row = contexts
        .iter()
        .find(|row| row.context_id == context_id)
        .expect("seeded context listed");
    assert_eq!(row.user_id, user_id);
    assert_eq!(row.task_count, 1);

    assert!(report.context_count(start, end).await.expect("count") >= 1);

    let unrelated = SessionId::generate();
    let found = report
        .agent_context_sessions(&[SessionId::new(context_id.as_str()), unrelated.clone()])
        .await
        .expect("agent context sessions");
    assert_eq!(found, vec![SessionId::new(context_id.as_str())]);
    assert!(!found.contains(&unrelated));
}

#[tokio::test]
async fn unknown_agent_has_no_activity() {
    let pool = test_db_pool().await;
    let report = AgentActivityReportRepository::new(&pool);
    let end = Utc::now();
    let count = report
        .agent_task_count("__no_such_report_agent__", end - Duration::days(1), end)
        .await
        .expect("agent task count");
    assert_eq!(count, 0);
    let summary = report
        .agent_summary("__no_such_report_agent__", end - Duration::days(1), end)
        .await
        .expect("agent summary");
    assert_eq!(summary.total_tasks, 0);
}

async fn seed_context_rollup(
    pool: &systemprompt_database::DbPool,
    user_id: &systemprompt_identifiers::UserId,
    context_id: &systemprompt_identifiers::ContextId,
    model: &str,
    agent_name: &str,
    last_activity: chrono::DateTime<Utc>,
) {
    let raw = pool.pool();
    sqlx::query(
        "INSERT INTO ai_context_rollup_hourly \
         (bucket, user_id, context_id, model, agent_name, traffic, status, request_count, last_activity) \
         VALUES (date_trunc('hour', $5, 'UTC'), $1, $2, $3, $4, 'production', 'completed', 1, $5)",
    )
    .bind(user_id.as_str())
    .bind(context_id.as_str())
    .bind(model)
    .bind(agent_name)
    .bind(last_activity)
    .execute(raw.as_ref())
    .await
    .expect("seed context rollup row");
}

#[tokio::test]
async fn recent_contexts_never_show_another_users_model_agent_or_name() {
    let pool = test_db_pool().await;
    let (owner, owner_session) = seed_user_and_session(&pool).await;
    let (other, _other_session) = seed_user_and_session(&pool).await;
    let (context_id, _task_id) = seed_context_and_task(&repos(&pool), &owner, &owner_session).await;
    let now = Utc::now();
    seed_context_rollup(&pool, &other, &context_id, "other-model", "other-agent", now - Duration::minutes(5)).await;
    seed_context_rollup(&pool, &owner, &context_id, "owner-model", "owner-agent", now - Duration::minutes(1)).await;
    let report = AgentActivityReportRepository::new(&pool);

    let rows = report
        .recent_contexts_for_user(&other, now + Duration::hours(1), 10)
        .await
        .expect("recent contexts");
    let row = rows
        .iter()
        .find(|row| row.context_id == context_id)
        .expect("the caller's own activity in the context is listed");

    assert_eq!(row.model.as_deref(), Some("other-model"));
    assert_eq!(row.agent_name.as_ref().map(|name| name.as_str()), Some("other-agent"));
    assert_eq!(row.context_name, None);
}
