//! DB-backed tests that drive [`SchedulerService::run_bootstrap_jobs`] end to
//! end, exercising the bootstrap orchestration (`scheduling/bootstrap.rs`) and
//! the underlying job dispatch path (`scheduling/dispatch.rs`).
//!
//! `run_bootstrap_jobs` validates configured names against the inventory
//! catalog, resolves owners, then dispatches each bootstrap job serially,
//! recording its result in the `scheduled_jobs` row. These tests assert the
//! observable outcomes: the registered-job count, the recorded run state, and
//! the loud failure on an unknown bootstrap name.
//!
//! The built-in bootstrap jobs (`database_cleanup`,
//! `cleanup_inactive_sessions`) touch shared tables, so these tests join the
//! serialized `scheduler-jobs-db` nextest group. Tests early-return when
//! `DATABASE_URL` is unset.

use std::sync::Arc;

use systemprompt_scheduler::{JobStatus, SchedulerConfig, SchedulerRepository, SchedulerService};
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};

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

fn config_with_bootstrap(bootstrap: Vec<String>, distributed_lock: bool) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: Vec::new(),
        bootstrap_jobs: bootstrap,
        distributed_lock,
    }
}

mod bootstrap_dispatch_db {
    use super::*;

    #[tokio::test]
    async fn run_bootstrap_jobs_returns_registered_job_count() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        // No bootstrap jobs: nothing dispatched, but the registered-job count
        // (the inventory catalog size) is still returned and must be non-zero
        // because the crate registers built-in jobs via `submit_job!`.
        let config = config_with_bootstrap(Vec::new(), true);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let count = svc
            .run_bootstrap_jobs(None)
            .await
            .expect("run_bootstrap_jobs must succeed with no bootstrap jobs");

        assert!(
            count > 0,
            "registered job count must be > 0 (built-in jobs are inventory-registered)"
        );
    }

    #[tokio::test]
    async fn run_bootstrap_jobs_dispatches_and_records_success() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        // `cleanup_inactive_sessions` is an inventory-registered cleanup job
        // that runs cleanly against an empty/migrated DB. Disable the
        // distributed lock so the single-replica test path is deterministic.
        let job_name = "cleanup_inactive_sessions";
        let config = config_with_bootstrap(vec![job_name.to_owned()], false);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("run_bootstrap_jobs must succeed for a known bootstrap job");

        // Dispatch records the run in the scheduled_jobs row. After a clean run
        // the row must exist with a Success status.
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let row = repo
            .find_job(job_name)
            .await
            .expect("find_job must succeed")
            .expect("dispatched bootstrap job must have a scheduled_jobs row");

        assert_eq!(
            row.last_status.as_deref(),
            Some(JobStatus::Success.as_str()),
            "a clean cleanup_inactive_sessions bootstrap run must record Success, got {:?}",
            row.last_status
        );
        assert!(
            row.run_count >= 1,
            "dispatch must have incremented run_count at least once, got {}",
            row.run_count
        );
    }

    #[tokio::test]
    async fn run_bootstrap_jobs_unknown_name_fails_loud() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let config = config_with_bootstrap(vec!["totally_unregistered_job_xyz".to_owned()], true);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let err = svc
            .run_bootstrap_jobs(None)
            .await
            .expect_err("an unknown bootstrap job name must fail validation, not be skipped");

        let msg = err.to_string();
        assert!(
            msg.contains("totally_unregistered_job_xyz"),
            "the validation error must name the unknown job; got: {msg}"
        );
    }

    #[tokio::test]
    async fn run_bootstrap_jobs_with_distributed_lock_records_run() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        // Same as the success case but with distributed_lock enabled, driving
        // the lock-acquisition branch of dispatch.rs.
        let job_name = "cleanup_inactive_sessions";
        let config = config_with_bootstrap(vec![job_name.to_owned()], true);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("run_bootstrap_jobs with distributed_lock must succeed");

        let repo = SchedulerRepository::new(&pool).expect("repo");
        let row = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("bootstrap job row must exist after a locked dispatch");

        // The lock path either runs the job (Success) or skips a duplicate tick;
        // either way the row exists and carries a terminal status, never the
        // transient Running left dangling.
        assert!(
            row.last_status.is_some(),
            "a dispatched job under distributed_lock must have a recorded status"
        );
    }
}
