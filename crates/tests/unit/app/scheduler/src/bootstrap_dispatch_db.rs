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

        // The bootstrap dispatch path only *updates* an existing scheduled_jobs
        // row — `record_run` returns early when the row is absent — mirroring a
        // job already registered by scheduling setup. Seed the row so this test
        // owns it rather than depending on a concurrent test having upserted the
        // same shared, inventory-registered job name.
        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(job_name, "0 0 * * * *", true)
            .await
            .expect("seed scheduled_jobs row");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("run_bootstrap_jobs must succeed for a known bootstrap job");

        // Dispatch records the run in the scheduled_jobs row. After a clean run
        // the row must exist with a Success status.
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

mod dispatch_outcome_arms {
    use super::*;
    use crate::test_jobs::{FAILING_JOB, PANIC_JOB};

    #[tokio::test]
    async fn panicking_job_records_failed_status_with_panic_message() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(PANIC_JOB, "", true)
            .await
            .expect("seed scheduled_jobs row");

        let config = config_with_bootstrap(vec![PANIC_JOB.to_owned()], false);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("a panicking job must not abort the bootstrap pass");

        let row = repo
            .find_job(PANIC_JOB)
            .await
            .expect("find_job")
            .expect("seeded row must exist");
        assert_eq!(
            row.last_status.as_deref(),
            Some(JobStatus::Failed.as_str()),
            "a panicking job must be recorded as Failed"
        );
        let error = row.last_error.expect("panic must record an error message");
        assert!(
            error.contains("deliberate test panic payload"),
            "the recorded error must carry the panic payload, got: {error}"
        );
        assert!(row.run_count >= 1, "run_count must have been incremented");
    }

    #[tokio::test]
    async fn failing_job_records_failed_status_with_its_message() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(FAILING_JOB, "", true)
            .await
            .expect("seed scheduled_jobs row");

        let config = config_with_bootstrap(vec![FAILING_JOB.to_owned()], false);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("a failing job must not abort the bootstrap pass");

        let row = repo
            .find_job(FAILING_JOB)
            .await
            .expect("find_job")
            .expect("seeded row must exist");
        assert_eq!(
            row.last_status.as_deref(),
            Some(JobStatus::Failed.as_str()),
            "JobResult{{success: false}} must be recorded as Failed"
        );
        assert_eq!(
            row.last_error.as_deref(),
            Some("deliberate test failure"),
            "the job's failure message must be recorded verbatim"
        );
    }

    #[tokio::test]
    async fn dispatch_without_scheduled_jobs_row_reports_missing_row() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let pg = pool.write_pool_arc().expect("write pool");
        sqlx::query!(
            "DELETE FROM scheduled_jobs WHERE job_name = $1",
            FAILING_JOB
        )
        .execute(&*pg)
        .await
        .expect("clear any pre-existing test-job row");

        let config = config_with_bootstrap(vec![FAILING_JOB.to_owned()], false);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        // Dispatch only UPDATEs the scheduled_jobs row; with no row present the
        // bootstrap completion probe hits its row-missing arm and the pass still
        // succeeds.
        svc.run_bootstrap_jobs(None)
            .await
            .expect("a missing scheduled_jobs row must not abort bootstrap");

        let repo = SchedulerRepository::new(&pool).expect("repo");
        assert!(
            repo.find_job(FAILING_JOB)
                .await
                .expect("find_job")
                .is_none(),
            "dispatch must not create a scheduled_jobs row it never upserted"
        );
    }
}

mod distributed_lock_arms {
    use super::*;

    #[tokio::test]
    async fn peer_held_advisory_lock_skips_the_dispatch() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let job_name = "cleanup_empty_contexts";
        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(job_name, "0 0 * * * *", true)
            .await
            .expect("seed scheduled_jobs row");
        let before = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("seeded row")
            .run_count;

        let pg = pool.write_pool_arc().expect("write pool");
        let mut peer = pg.acquire().await.expect("peer connection");
        let key: i64 = sqlx::query_scalar!(r#"SELECT hashtext($1)::bigint AS "key!""#, job_name)
            .fetch_one(peer.as_mut())
            .await
            .expect("hash job name");
        let acquired: Option<bool> =
            sqlx::query_scalar!(r#"SELECT pg_try_advisory_lock($1) AS "acquired""#, key)
                .fetch_one(peer.as_mut())
                .await
                .expect("peer lock");
        assert_eq!(
            acquired,
            Some(true),
            "peer must win the advisory lock first"
        );

        let config = config_with_bootstrap(vec![job_name.to_owned()], true);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");
        svc.run_bootstrap_jobs(None)
            .await
            .expect("a lock-skipped dispatch must not abort bootstrap");

        let after = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("row still present")
            .run_count;
        assert_eq!(
            after, before,
            "a dispatch skipped by a peer-held advisory lock must not increment run_count"
        );

        sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", key)
            .fetch_one(peer.as_mut())
            .await
            .expect("peer unlock");
    }

    #[tokio::test]
    async fn fresh_last_run_deduplicates_the_tick() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let job_name = "cleanup_empty_contexts";
        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(job_name, "0 0 * * * *", true)
            .await
            .expect("seed scheduled_jobs row");
        repo.update_job_execution(job_name, JobStatus::Success, None, None)
            .await
            .expect("stamp last_run = now");
        let before = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("seeded row")
            .run_count;

        let config = config_with_bootstrap(vec![job_name.to_owned()], true);
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");
        svc.run_bootstrap_jobs(None)
            .await
            .expect("a tick-deduplicated dispatch must not abort bootstrap");

        let row = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("row still present");
        assert_eq!(
            row.run_count, before,
            "a peer-completed tick within 900ms must be skipped, not re-run"
        );
        assert_eq!(
            row.last_status.as_deref(),
            Some(JobStatus::Success.as_str()),
            "a skipped dispatch must not overwrite the recorded status"
        );
    }
}

mod closed_pool_resilience {
    use super::*;
    use systemprompt_test_fixtures::closed_db_pool;

    #[tokio::test]
    async fn bootstrap_survives_a_dead_database_without_lock() {
        let (real_pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&real_pool, &url).expect("fixture AppContext");
        let closed = closed_db_pool().await;

        // Every repository write (Running transition, run-count increment,
        // failure recording) and the job body itself fail against the closed
        // pool; dispatch logs each error and the bootstrap pass still succeeds.
        let config = config_with_bootstrap(vec!["cleanup_inactive_sessions".to_owned()], false);
        let svc = SchedulerService::new(config, closed, app_ctx).expect("SchedulerService::new");

        let count = svc
            .run_bootstrap_jobs(None)
            .await
            .expect("bootstrap must degrade gracefully when the DB is unreachable");
        assert!(count > 0, "the inventory catalog is DB-independent");
    }

    #[tokio::test]
    async fn bootstrap_survives_a_dead_database_with_lock() {
        let (real_pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&real_pool, &url).expect("fixture AppContext");
        let closed = closed_db_pool().await;

        // With distributed_lock enabled, the advisory-lock acquisition fails on
        // the closed pool and the dispatch is skipped rather than run.
        let config = config_with_bootstrap(vec!["cleanup_inactive_sessions".to_owned()], true);
        let svc = SchedulerService::new(config, closed, app_ctx).expect("SchedulerService::new");

        let count = svc
            .run_bootstrap_jobs(None)
            .await
            .expect("a failed lock acquisition must skip the job, not abort bootstrap");
        assert!(count > 0);
    }
}

mod bootstrap_owner_arms {
    use super::*;
    use systemprompt_identifiers::UserId;
    use systemprompt_scheduler::JobConfig;

    #[tokio::test]
    async fn bootstrap_job_with_unresolved_owner_is_skipped() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let job_name = "cleanup_empty_contexts";
        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(job_name, "0 0 * * * *", true)
            .await
            .expect("seed scheduled_jobs row");
        let before = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("seeded row")
            .run_count;

        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![
                JobConfig::new(job_name)
                    .with_owner(UserId::new("sp-test-no-such-owner"))
                    .with_schedule("0 0 4 * * *"),
            ],
            bootstrap_jobs: vec![job_name.to_owned()],
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("an unresolved bootstrap owner must skip the job, not abort");

        let after = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("row still present")
            .run_count;
        assert_eq!(
            after, before,
            "a bootstrap job with an unresolved owner must not be dispatched"
        );
    }

    #[tokio::test]
    async fn bootstrap_job_with_active_explicit_owner_runs_as_that_owner() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");
        let pg = pool.write_pool_arc().expect("write pool");

        let owner_name = format!("sp_test_owner_{}", std::process::id());
        let owner_id = format!("sp-test-owner-id-{}", std::process::id());
        let email = format!("{owner_name}@test.invalid");
        sqlx::query!(
            "INSERT INTO users (id, name, email, status) VALUES ($1, $2, $3, 'active')
             ON CONFLICT (name) DO UPDATE SET status = 'active'",
            owner_id,
            owner_name,
            email,
        )
        .execute(&*pg)
        .await
        .expect("seed active owner user");

        let job_name = "cleanup_empty_contexts";
        let repo = SchedulerRepository::new(&pool).expect("repo");
        repo.upsert_job(job_name, "0 0 * * * *", true)
            .await
            .expect("seed scheduled_jobs row");
        let before = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("seeded row")
            .run_count;

        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![
                JobConfig::new(job_name)
                    .with_owner(UserId::new(owner_name.as_str()))
                    .with_schedule("0 0 4 * * *"),
            ],
            bootstrap_jobs: vec![job_name.to_owned()],
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        svc.run_bootstrap_jobs(None)
            .await
            .expect("a resolvable explicit owner must dispatch normally");

        let after = repo
            .find_job(job_name)
            .await
            .expect("find_job")
            .expect("row still present")
            .run_count;
        assert_eq!(
            after,
            before + 1,
            "the owned bootstrap job must have been dispatched exactly once"
        );

        sqlx::query!("DELETE FROM users WHERE id = $1", owner_id)
            .execute(&*pg)
            .await
            .expect("cleanup owner user");
    }
}
