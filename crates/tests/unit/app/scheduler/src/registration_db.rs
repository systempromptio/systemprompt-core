//! Cron registration in [`SchedulerService::start`]: a job whose schedule
//! does not parse is skipped, reported as degraded and logged as an ERROR
//! while its siblings still register; a job the deployment has not
//! configured is never scheduled; and a registered entry actually fires the
//! job through the cron loop. These tests write `scheduled_jobs`/`logs` and
//! join the serialized `scheduler-jobs-db` nextest group.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::JobName;
use systemprompt_logging::{LogLevel, LoggingRepository};
use systemprompt_manifest::services::UnlistedJobs;
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerService, SchedulerStartup};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_traits::{Job, JobContext, JobResult, JobScope};

const TICK_JOB: &str = "sp_test_registration_tick_job";
const UNCONFIGURED_JOB: &str = "sp_test_registration_unconfigured_job";

static TICK_RUNS: AtomicU64 = AtomicU64::new(0);
static UNCONFIGURED_RUNS: AtomicU64 = AtomicU64::new(0);

struct TickJob;

#[async_trait]
impl Job for TickJob {
    fn name(&self) -> &'static str {
        TICK_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    fn scope(&self) -> JobScope {
        JobScope::Node
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        TICK_RUNS.fetch_add(1, Ordering::SeqCst);
        Ok(JobResult::success())
    }
}

struct UnconfiguredJob;

#[async_trait]
impl Job for UnconfiguredJob {
    fn name(&self) -> &'static str {
        UNCONFIGURED_JOB
    }

    fn schedule(&self) -> &'static str {
        "* * * * * *"
    }

    fn enabled(&self) -> bool {
        false
    }

    fn configured(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        UNCONFIGURED_RUNS.fetch_add(1, Ordering::SeqCst);
        Ok(JobResult::success())
    }
}

systemprompt_provider_contracts::submit_job!(&TickJob);
systemprompt_provider_contracts::submit_job!(&UnconfiguredJob);

fn config(jobs: Vec<JobConfig>) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs,
        bootstrap_jobs: Vec::new(),
        distributed_lock: false,
        unlisted_jobs: UnlistedJobs::Warn,
    }
}

async fn start(jobs: Vec<JobConfig>) -> (SchedulerStartup, DbPool) {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    let svc = SchedulerService::new(config(jobs), Arc::clone(&pool), app_ctx);
    let startup = svc.start().await.expect("scheduler starts");
    (startup, pool)
}

mod registration_db {
    use super::*;

    #[tokio::test]
    async fn an_unparseable_schedule_skips_only_that_job_and_is_logged() {
        let (startup, pool) = start(vec![
            JobConfig::new(JobName::new("cleanup_inactive_sessions")).with_schedule("0 0 4 * * *"),
            JobConfig::new(JobName::new("cleanup_empty_contexts"))
                .with_schedule("definitely not cron"),
        ])
        .await;

        assert_eq!(startup.scheduled, 1, "the valid sibling still registers");
        assert_eq!(startup.degraded.len(), 1, "{:?}", startup.degraded);
        let skipped = &startup.degraded[0];
        assert_eq!(skipped.job_name, "cleanup_empty_contexts");
        assert_eq!(skipped.owner, "system admin");
        assert!(!skipped.reason.is_empty());

        let logs = LoggingRepository::new(&pool)
            .get_logs_by_module_patterns(&["scheduler".to_owned()], 1000)
            .await
            .expect("query scheduler logs");
        assert!(
            logs.iter()
                .any(|entry| matches!(entry.level, LogLevel::Error)
                    && entry.message.contains("cleanup_empty_contexts")
                    && entry.message.contains("could not be registered")),
            "the skipped job must be recorded as an ERROR"
        );
        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }
    }

    #[tokio::test]
    async fn a_job_this_deployment_has_not_configured_is_never_scheduled() {
        let (startup, _pool) = start(vec![
            JobConfig::new(JobName::new(UNCONFIGURED_JOB)).with_schedule("* * * * * *"),
        ])
        .await;

        assert_eq!(startup.scheduled, 0);
        assert!(startup.degraded.is_empty(), "{:?}", startup.degraded);
        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }
        assert_eq!(UNCONFIGURED_RUNS.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_registered_entry_fires_the_job_from_the_cron_loop() {
        let before = TICK_RUNS.load(Ordering::SeqCst);
        let (startup, _pool) = start(vec![
            JobConfig::new(JobName::new(TICK_JOB)).with_schedule("* * * * * *"),
        ])
        .await;
        assert_eq!(startup.scheduled, 1);

        let fired = tokio::time::timeout(Duration::from_secs(15), async {
            while TICK_RUNS.load(Ordering::SeqCst) == before {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }
        assert!(
            fired.is_ok(),
            "the every-second schedule never fired the job"
        );
    }
}
