//! DB-backed tests for scheduler job `execute` bodies.
//!
//! Each job is constructed (zero-cost unit struct), a real `JobContext` is
//! assembled from the fixture pool, and `execute` is driven against the
//! migrated DB. The DB starts empty of application data, so every job should
//! complete without errors and return a success `JobResult`. Tests fail when
//! `DATABASE_URL` is unset.

use std::sync::Arc;

use systemprompt_scheduler::DatabaseCleanupJob;
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_traits::{Dependencies, Job, JobContext};

fn make_test_ctx(pool: &systemprompt_database::DbPool, url: &str) -> JobContext {
    use systemprompt_identifiers::{Actor, UserId};

    let app_ctx = test_app_context(pool, url);

    let dependencies = Dependencies::new()
        .with(Arc::clone(app_ctx.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(app_ctx);

    let owner = UserId::new("job-test-admin");
    let actor = Actor::job(owner, "test".to_string());

    JobContext::new(actor, dependencies)
}

mod database_cleanup_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let url = test_database_url();
        let pool = test_db_pool().await;
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
        let url = test_database_url();
        let pool = test_db_pool().await;
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
        let url = test_database_url();
        let pool = test_db_pool().await;
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

mod closed_pool_error_propagation {
    use super::*;

    async fn make_closed_ctx(url: &str) -> JobContext {
        use systemprompt_identifiers::{Actor, UserId};
        use systemprompt_test_fixtures::closed_db_pool;

        let real_pool = test_db_pool().await;
        let app_ctx = test_app_context(&real_pool, url);
        let closed = closed_db_pool().await;

        let dependencies = Dependencies::new()
            .with(Arc::clone(app_ctx.app_paths_arc()))
            .with(closed)
            .with(app_ctx);

        let actor = Actor::job(UserId::new("job-test-admin"), "test".to_string());
        JobContext::new(actor, dependencies)
    }

    #[tokio::test]
    async fn database_cleanup_propagates_a_dead_pool_error() {
        let url = test_database_url();
        let ctx = make_closed_ctx(&url).await;

        DatabaseCleanupJob
            .execute(&ctx)
            .await
            .expect_err("a dead pool must surface as an execute error, not success");
    }
}
