//! Job::execute() smoke tests that exercise the real query paths against the
//! test Postgres database. Each test wires the job's required dependencies
//! (DbPool / AppPaths) into a JobContext and asserts that the job reports
//! success.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_scheduler::{CleanupInactiveSessionsJob, DatabaseCleanupJob};
use systemprompt_test_fixtures::{
    fixture_actor, test_app_context, test_database_url, test_db_pool,
};
use systemprompt_traits::{Dependencies, Job, JobContext};

fn make_ctx(pool: &DbPool) -> JobContext {
    JobContext::new(fixture_actor(), Dependencies::new().with(Arc::clone(pool)))
}

// database_cleanup reads its per-table windows from `profile.retention`, so it
// needs a real AppContext rather than the unit placeholder `make_ctx` supplies.
async fn make_ctx_with_app(pool: &DbPool) -> JobContext {
    let url = test_database_url();
    let app = test_app_context(pool, &url);
    JobContext::new(
        fixture_actor(),
        Dependencies::new().with(Arc::clone(pool)).with(app),
    )
}

#[tokio::test]
async fn database_cleanup_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = make_ctx_with_app(&pool).await;
    let result = DatabaseCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
    let message = result.message.as_deref().expect("job records what it did");
    for table in [
        "orphaned_logs",
        "logs",
        "analytics_events",
        "ai_request_messages",
        "mcp_tool_executions",
        "event_outbox",
        "ai_request_payloads",
        "governance_decisions",
        "ai_quota_buckets",
        "engagement_events",
        "fingerprint_reputation",
    ] {
        assert!(
            message.contains(&format!("{table}=")),
            "retention pass for {table} missing from {message}"
        );
    }
}

#[tokio::test]
async fn database_cleanup_job_without_an_app_context_fails() {
    let pool = test_db_pool().await;
    let ctx = make_ctx(&pool);
    let error = DatabaseCleanupJob
        .execute(&ctx)
        .await
        .expect_err("retention windows come from the profile, so there is nothing to fall back to");
    assert!(
        error.to_string().contains("AppContext"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn cleanup_inactive_sessions_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = make_ctx(&pool);
    let result = CleanupInactiveSessionsJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn jobs_fail_when_dbpool_missing() {
    let ctx = JobContext::new(fixture_actor(), Dependencies::new());

    assert!(DatabaseCleanupJob.execute(&ctx).await.is_err());
    assert!(CleanupInactiveSessionsJob.execute(&ctx).await.is_err());
}
