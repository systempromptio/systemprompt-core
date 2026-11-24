//! DB-backed runs of the scheduler's retention job through the scheduler's
//! public execution path, which hands a scheduler-owned job its kernel
//! handles. Against a database with nothing to clean up every run succeeds
//! with no failures; against a closed pool the run fails at the database.

use systemprompt_test_fixtures::{AppContextSeeds as _, DisposableDb, test_database_url};

use crate::test_extension::{job_service, retention_job, run_admitted_job, scheduled_app};

mod database_cleanup_db {
    use super::*;

    #[tokio::test]
    async fn execute_succeeds_against_empty_db() {
        let (app, schedule) = scheduled_app(&test_database_url()).await;

        let result = run_admitted_job(app, schedule, retention_job(false, &[])).await;

        assert!(
            result.success,
            "DatabaseCleanupJob must report success even with nothing to clean up: {:?}",
            result.message
        );
    }

    #[tokio::test]
    async fn execute_is_idempotent() {
        let (app, schedule) = scheduled_app(&test_database_url()).await;
        let job = retention_job(false, &[]);
        let name = job.name.clone();
        let service = job_service(app, schedule, vec![job]);

        for _ in 0..2 {
            let result = service
                .run_job(&name, &std::collections::HashMap::new())
                .await;
            assert!(
                result.success,
                "repeated DatabaseCleanupJob executions must all succeed: {:?}",
                result.message
            );
        }
    }

    #[tokio::test]
    async fn execute_reports_no_failures() {
        let (app, schedule) = scheduled_app(&test_database_url()).await;

        let result = run_admitted_job(app, schedule, retention_job(false, &[])).await;

        assert!(result.success, "{:?}", result.message);
        assert_eq!(
            result.items_failed.unwrap_or(0),
            0,
            "DatabaseCleanupJob must report 0 failures on an empty DB"
        );
    }
}

mod closed_pool_error_propagation {
    use super::*;

    #[tokio::test]
    async fn database_cleanup_propagates_a_dead_pool_error() {
        let database = DisposableDb::with_schema("sched_cleanup_dead_pool").await;
        let (app, schedule) = scheduled_app(database.url()).await;
        app.db_pool().write_pool().close().await;
        app.db_pool().pool().close().await;

        let result = run_admitted_job(app, schedule, retention_job(false, &[])).await;

        assert!(
            !result.success,
            "a dead pool must surface as a failed run, not success"
        );
        let message = result.message.unwrap_or_default();
        assert!(
            !message.contains("KernelJobHandles"),
            "the run reaches the database: {message}"
        );
        database.drop_now().await;
    }
}
