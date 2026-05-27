//! Job::execute() smoke tests that exercise the real query paths against the
//! test Postgres database. Each test wires the job's required dependencies
//! (DbPool / AppPaths) into a JobContext and asserts that the job reports
//! success.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_scheduler::{
    BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
    DatabaseCleanupJob, GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob,
};
use systemprompt_test_fixtures::{fixture_actor, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};

async fn try_pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn make_ctx(pool: &DbPool) -> JobContext {
    let pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(pool));
    let ctx_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let paths_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    JobContext::new(fixture_actor(), pool_any, ctx_any, paths_any)
}

#[tokio::test]
async fn database_cleanup_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = DatabaseCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn cleanup_inactive_sessions_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = CleanupInactiveSessionsJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn cleanup_empty_contexts_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = CleanupEmptyContextsJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn behavioral_analysis_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = BehavioralAnalysisJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn ghost_session_cleanup_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = GhostSessionCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn malicious_ip_blacklist_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = MaliciousIpBlacklistJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn no_js_cleanup_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = NoJsCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn jobs_fail_when_dbpool_missing() {
    let pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let ctx_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let paths_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let ctx = JobContext::new(fixture_actor(), pool_any, ctx_any, paths_any);

    assert!(DatabaseCleanupJob.execute(&ctx).await.is_err());
    assert!(CleanupInactiveSessionsJob.execute(&ctx).await.is_err());
    assert!(CleanupEmptyContextsJob.execute(&ctx).await.is_err());
    assert!(BehavioralAnalysisJob.execute(&ctx).await.is_err());
    assert!(GhostSessionCleanupJob.execute(&ctx).await.is_err());
    assert!(MaliciousIpBlacklistJob.execute(&ctx).await.is_err());
    assert!(NoJsCleanupJob.execute(&ctx).await.is_err());
}
