//! DB-backed tests for scheduler job `execute` bodies.
//!
//! Each job is constructed (zero-cost unit struct), a real `JobContext` is
//! assembled from the fixture pool, and `execute` is driven against the
//! migrated DB. The DB starts empty of application data, so every job should
//! complete without errors and return a success `JobResult`. Tests
//! early-return when `DATABASE_URL` is unset.

use std::sync::Arc;

use systemprompt_scheduler::{BehavioralAnalysisJob, DatabaseCleanupJob, MaliciousIpBlacklistJob};
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};

macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        (pool, url)
    }};
}

fn make_test_ctx(
    pool: &systemprompt_database::DbPool,
    url: &str,
) -> JobContext {
    use systemprompt_identifiers::{Actor, UserId};

    let app_ctx = fixture_app_context(pool, url)
        .expect("fixture AppContext must build against a migrated DB");

    // Why: JobContext stores type-erased Arcs; jobs downcast to the concrete
    // type. The production make_job_context wraps each value in Arc::new so
    // the downcast target is the original concrete type.
    let app_paths_any: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(Arc::clone(app_ctx.app_paths_arc()));
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(pool));
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(app_ctx);

    let owner = UserId::new("job-test-admin");
    let actor = Actor::job(owner, "test".to_string());

    JobContext::new(actor, db_pool_any, app_context_any, app_paths_any)
}

mod behavioral_analysis_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = BehavioralAnalysisJob;

        let result = job
            .execute(&ctx)
            .await
            .expect("BehavioralAnalysisJob::execute must not error on an empty DB");

        assert!(
            result.success,
            "BehavioralAnalysisJob must report success when no fingerprints require action"
        );
    }

    #[tokio::test]
    async fn execute_returns_valid_duration() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = BehavioralAnalysisJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        let _ = result.duration_ms;
    }

    #[tokio::test]
    async fn execute_has_stats_fields() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = BehavioralAnalysisJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        assert_eq!(
            result.items_failed.unwrap_or(0),
            0,
            "BehavioralAnalysisJob must report 0 failures on an empty DB"
        );
    }
}

mod malicious_ip_blacklist_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = MaliciousIpBlacklistJob;

        let result = job
            .execute(&ctx)
            .await
            .expect("MaliciousIpBlacklistJob::execute must not error on an empty DB");

        assert!(
            result.success,
            "MaliciousIpBlacklistJob must report success when no candidates are found"
        );
    }

    #[tokio::test]
    async fn execute_reports_zero_banned_on_empty_db() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = MaliciousIpBlacklistJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        assert_eq!(
            result.items_processed.unwrap_or(0),
            0,
            "no IPs should be banned against an empty DB"
        );
    }

    #[tokio::test]
    async fn execute_is_idempotent() {
        let (pool, url) = pool_or_skip!();
        let job = MaliciousIpBlacklistJob;

        for _ in 0..2 {
            let ctx = make_test_ctx(&pool, &url);
            job.execute(&ctx)
                .await
                .expect("repeated MaliciousIpBlacklistJob executions must all succeed");
        }
    }
}

mod database_cleanup_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = DatabaseCleanupJob;

        let result = job
            .execute(&ctx)
            .await
            .expect("DatabaseCleanupJob::execute must not error on an empty DB");

        assert!(
            result.success,
            "DatabaseCleanupJob must report success even with nothing to clean up"
        );
    }

    #[tokio::test]
    async fn execute_is_idempotent() {
        let (pool, url) = pool_or_skip!();
        let job = DatabaseCleanupJob;

        for _ in 0..2 {
            let ctx = make_test_ctx(&pool, &url);
            job.execute(&ctx)
                .await
                .expect("repeated DatabaseCleanupJob executions must all succeed");
        }
    }

    #[tokio::test]
    async fn execute_reports_no_failures() {
        let (pool, url) = pool_or_skip!();
        let ctx = make_test_ctx(&pool, &url);
        let job = DatabaseCleanupJob;

        let result = job.execute(&ctx).await.expect("execute must not error");

        assert_eq!(
            result.items_failed.unwrap_or(0),
            0,
            "DatabaseCleanupJob must report 0 failures on an empty DB"
        );
    }
}
