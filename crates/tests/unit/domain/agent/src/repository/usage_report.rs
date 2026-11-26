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
