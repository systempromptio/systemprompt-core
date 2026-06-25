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
