//! `execute` smoke tests for the analytics jobs against the test database,
//! and their refusal to run without the dependencies the scheduler injects.

use systemprompt_analytics::jobs::{
    BehavioralAnalysisJob, GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob,
};
use systemprompt_test_fixtures::{fixture_actor, test_db_pool};
use systemprompt_traits::{Dependencies, Job, JobContext};

use super::job_ctx;

#[tokio::test]
async fn behavioral_analysis_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = job_ctx(&pool);
    let result = BehavioralAnalysisJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn ghost_session_cleanup_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = job_ctx(&pool);
    let result = GhostSessionCleanupJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn malicious_ip_blacklist_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = job_ctx(&pool);
    let result = MaliciousIpBlacklistJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn no_js_cleanup_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = job_ctx(&pool);
    let result = NoJsCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn jobs_fail_when_dependencies_are_missing() {
    let ctx = JobContext::new(fixture_actor(), Dependencies::new());
    assert!(BehavioralAnalysisJob.execute(&ctx).await.is_err());
    assert!(GhostSessionCleanupJob.execute(&ctx).await.is_err());
    assert!(MaliciousIpBlacklistJob.execute(&ctx).await.is_err());
    assert!(NoJsCleanupJob.execute(&ctx).await.is_err());
}

#[tokio::test]
async fn ip_ban_detectors_fail_without_a_ban_store() {
    let pool = test_db_pool().await;
    let ctx = JobContext::new(
        fixture_actor(),
        Dependencies::new().with(std::sync::Arc::clone(&pool)),
    );
    assert!(BehavioralAnalysisJob.execute(&ctx).await.is_err());
    assert!(MaliciousIpBlacklistJob.execute(&ctx).await.is_err());
}
