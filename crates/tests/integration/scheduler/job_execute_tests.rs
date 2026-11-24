//! Job smoke tests that exercise the real query paths against the test
//! Postgres database. A job that needs only a database runs under a
//! JobContext carrying the pool; the scheduler-owned retention job runs
//! through the scheduler's execution service, the one path that hands it its
//! kernel handles, and a hand-built context without them is refused.

use std::collections::HashMap;

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, JobName};
use systemprompt_scheduler::{DatabaseCleanupJob, JobConfig, JobExecutionService, SchedulerConfig};
use systemprompt_test_fixtures::{test_db_pool, unique_user_id};
use systemprompt_traits::{Dependencies, Job, JobContext};

fn make_ctx(pool: &DbPool) -> JobContext {
    JobContext::new(
        Actor::user(unique_user_id("job-execute-tests")),
        Dependencies::new().with(Arc::clone(pool)),
    )
}

#[tokio::test]
async fn database_cleanup_job_execute_succeeds() {
    let (app, schedule) = crate::test_extension::app_with_test_jobs().await;
    let name = JobName::new(DatabaseCleanupJob.name());
    let mut config = SchedulerConfig::with_system_admin();
    config.jobs = vec![JobConfig::new(name.clone())];
    config.distributed_lock = false;
    let result = JobExecutionService::new(app, config, schedule)
        .expect("job execution service")
        .run_job(&name, &HashMap::new())
        .await;
    assert!(result.success, "job runs: {:?}", result.message);
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
async fn database_cleanup_job_without_its_kernel_handles_fails() {
    let pool = test_db_pool().await;
    let ctx = make_ctx(&pool);
    let error = DatabaseCleanupJob
        .execute(&ctx)
        .await
        .expect_err("retention windows come from the profile, so there is nothing to fall back to");
    assert!(
        error.to_string().contains("KernelJobHandles"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn jobs_fail_when_dbpool_missing() {
    let ctx = JobContext::new(
        Actor::user(unique_user_id("job-execute-tests")),
        Dependencies::new(),
    );

    assert!(DatabaseCleanupJob.execute(&ctx).await.is_err());
}
