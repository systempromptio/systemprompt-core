//! DB-backed tests for owner resolution in [`SchedulerService::start`].
//!
//! A job with no explicit owner runs as the profile `system_admin`; a job whose
//! explicit owner does not resolve to an active user is skipped (not fatal) and
//! recorded as an ERROR in the `logs` table. These tests touch the shared
//! `scheduled_jobs`/`logs` tables and join the serialized `scheduler-jobs-db`
//! nextest group. Tests early-return when `DATABASE_URL` is unset.

use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_logging::{LogLevel, LoggingRepository};
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerService};
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

mod start_owner_resolution_db {
    use super::*;

    #[tokio::test]
    async fn ownerless_job_resolves_to_system_admin_and_starts() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![JobConfig::new("cleanup_inactive_sessions").with_schedule("0 0 4 * * *")],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let startup = svc.start().await.expect("scheduler must start");
        assert!(startup.handle.is_some(), "scheduler must start");
        assert!(
            startup.degraded.is_empty(),
            "an ownerless job defaults to system_admin and must not be degraded"
        );

        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }
    }

    #[tokio::test]
    async fn unresolved_explicit_owner_is_skipped_and_logged() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let bad_owner = "no-such-active-user-zzz";
        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![
                JobConfig::new("cleanup_inactive_sessions").with_schedule("0 0 4 * * *"),
                JobConfig::new("cleanup_empty_contexts")
                    .with_owner(UserId::new(bad_owner))
                    .with_schedule("0 0 4 * * *"),
            ],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let startup = svc
            .start()
            .await
            .expect("scheduler must still start when one explicit owner is unresolved");
        assert!(
            startup.handle.is_some(),
            "one bad owner must not abort the scheduler"
        );
        assert_eq!(
            startup.degraded.len(),
            1,
            "exactly the bad-owner job is degraded, got {:?}",
            startup.degraded
        );
        let skipped = &startup.degraded[0];
        assert_eq!(skipped.job_name, "cleanup_empty_contexts");
        assert_eq!(skipped.owner, bad_owner);

        let logs = LoggingRepository::new(&pool)
            .expect("logging repository")
            .get_logs_by_module_patterns(&["scheduler".to_owned()], 1000)
            .await
            .expect("query scheduler logs");
        assert!(
            logs.iter()
                .any(|entry| matches!(entry.level, LogLevel::Error)
                    && entry.message.contains("cleanup_empty_contexts")
                    && entry.message.contains(bad_owner)),
            "an ERROR row naming the skipped job and owner must be persisted to logs"
        );

        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }
    }
}

mod start_lifecycle_db {
    use super::*;
    use systemprompt_scheduler::SchedulerRepository;

    #[tokio::test]
    async fn disabled_scheduler_returns_no_handle() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let config = SchedulerConfig {
            enabled: false,
            jobs: vec![JobConfig::new("cleanup_inactive_sessions")],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let startup = svc
            .start()
            .await
            .expect("a disabled scheduler must not error");
        assert!(
            startup.handle.is_none(),
            "disabled scheduler must yield no handle"
        );
        assert!(startup.degraded.is_empty());
    }

    #[tokio::test]
    async fn unknown_configured_job_fails_start_loud() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![JobConfig::new("sp_no_such_job_qqq").with_schedule("0 0 4 * * *")],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let err = svc
            .start()
            .await
            .expect_err("a configured job absent from the inventory must fail start");
        assert!(
            err.to_string().contains("sp_no_such_job_qqq"),
            "the error must name the unknown job, got: {err}"
        );
    }

    #[tokio::test]
    async fn disabled_job_config_is_not_registered_or_upserted() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");
        let pg = pool.write_pool_arc().expect("write pool");

        let job_name = crate::test_jobs::EMPTY_SCHEDULE_JOB;
        sqlx::query!("DELETE FROM scheduled_jobs WHERE job_name = $1", job_name)
            .execute(&*pg)
            .await
            .expect("clear test-job row");

        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![
                JobConfig::new(job_name)
                    .with_schedule("0 0 4 * * *")
                    .disabled(),
            ],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let startup = svc.start().await.expect("start must succeed");
        let repo = SchedulerRepository::new(&pool).expect("repo");
        assert!(
            repo.find_job(job_name).await.expect("find_job").is_none(),
            "a disabled job config must not be upserted into scheduled_jobs"
        );

        if let Some(handle) = startup.handle {
            assert!(format!("{handle:?}").contains("SchedulerHandle"));
            handle.shutdown().await.expect("shutdown");
        }
    }

    #[tokio::test]
    async fn empty_schedule_job_is_bootstrap_only_and_not_upserted() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");
        let pg = pool.write_pool_arc().expect("write pool");

        let job_name = crate::test_jobs::EMPTY_SCHEDULE_JOB;
        sqlx::query!("DELETE FROM scheduled_jobs WHERE job_name = $1", job_name)
            .execute(&*pg)
            .await
            .expect("clear test-job row");

        // No config schedule and an empty trait schedule: the job is
        // bootstrap/manual-only and start() must not cron-register or upsert it.
        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![JobConfig::new(job_name)],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let startup = svc.start().await.expect("start must succeed");
        let repo = SchedulerRepository::new(&pool).expect("repo");
        assert!(
            repo.find_job(job_name).await.expect("find_job").is_none(),
            "an empty-schedule job must not gain a scheduled_jobs row from start()"
        );

        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }
    }

    #[tokio::test]
    async fn overlapping_cron_ticks_skip_while_job_is_running() {
        let (pool, url) = pool_or_skip!();
        let app_ctx = fixture_app_context(&pool, &url).expect("fixture AppContext");

        use std::sync::atomic::Ordering;
        use std::time::{Duration, Instant};

        use crate::test_jobs::{SLOW_JOB, SLOW_JOB_STARTS};

        let config = SchedulerConfig {
            enabled: true,
            jobs: vec![JobConfig::new(SLOW_JOB).with_schedule("* * * * * *")],
            bootstrap_jobs: Vec::new(),
            distributed_lock: false,
        };
        let svc = SchedulerService::new(config, Arc::clone(&pool), app_ctx)
            .expect("SchedulerService::new");

        let startup = svc.start().await.expect("start must succeed");
        let handle = startup
            .handle
            .expect("enabled scheduler must yield a handle");

        // Wait for the first tick to start the 4s job, then let two more 1s
        // ticks fire while it is still running: the in-process RunningJobs
        // guard must skip them.
        let deadline = Instant::now() + Duration::from_secs(10);
        while SLOW_JOB_STARTS.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline, "first cron tick never fired");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_millis(2200)).await;
        assert_eq!(
            SLOW_JOB_STARTS.load(Ordering::SeqCst),
            1,
            "ticks landing while the job is still running must be skipped"
        );

        handle.shutdown().await.expect("shutdown");
    }
}
