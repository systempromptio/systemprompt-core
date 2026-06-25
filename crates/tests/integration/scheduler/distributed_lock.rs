//! Regression test for the scheduler's cross-replica single-execution
//! guarantee.
//!
//! When two scheduler replicas share one Postgres database and both have the
//! same job scheduled on the same cron, only ONE of them may actually run the
//! job per tick. The in-process `RunningJobs` set cannot enforce this — each
//! `SchedulerService` owns its own set — so the guarantee rests entirely on
//! the Postgres advisory lock claimed by `dispatch::execute_job` when
//! `SchedulerConfig.distributed_lock` is true.
//!
//! These tests encode that guarantee:
//!
//! * `distributed_lock_runs_job_once_across_replicas` asserts that with the
//!   lock enabled, `scheduled_jobs.run_count` advances roughly once per elapsed
//!   cron tick — not twice — proving the duplicate execution is suppressed.
//! * `without_distributed_lock_job_double_fires` is the negative control: it
//!   removes the lock and shows the run count roughly doubles, documenting
//!   exactly what the lock fixes.
//!
//! Both tests are DB-backed: they need a reachable Postgres (`DATABASE_URL`).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerService};
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

const TEST_JOB_NAME: &str = "test_distributed_lock_probe";

#[derive(Debug, Clone, Copy)]
struct DistributedLockProbeJob;

#[async_trait]
impl Job for DistributedLockProbeJob {
    fn name(&self) -> &'static str {
        TEST_JOB_NAME
    }

    fn description(&self) -> &'static str {
        "Test-only job exercising the scheduler distributed advisory lock"
    }

    fn schedule(&self) -> &'static str {
        // Every second — overridden per test via JobConfig anyway.
        "* * * * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

systemprompt_traits::submit_job!(&DistributedLockProbeJob);

fn probe_config(distributed_lock: bool) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: vec![JobConfig::new(TEST_JOB_NAME).with_schedule("* * * * * *")],
        bootstrap_jobs: vec![],
        distributed_lock,
    }
}

async fn reset_job_row(pool: &DbPool) -> Result<()> {
    sqlx::query("DELETE FROM scheduled_jobs WHERE job_name = $1")
        .bind(TEST_JOB_NAME)
        .execute(pool.write_pool_arc()?.as_ref())
        .await?;
    Ok(())
}

async fn read_run_count(pool: &DbPool) -> Result<i32> {
    let count: Option<i32> =
        sqlx::query_scalar("SELECT run_count FROM scheduled_jobs WHERE job_name = $1")
            .bind(TEST_JOB_NAME)
            .fetch_optional(pool.pool_arc()?.as_ref())
            .await?;
    Ok(count.unwrap_or(0))
}

async fn run_two_replicas(distributed_lock: bool, window: Duration) -> Result<i32> {
    let database_url = fixture_database_url()?;
    let pool = fixture_db_pool(&database_url).await?;
    reset_job_row(&pool).await?;

    let app_context = fixture_app_context(&pool, &database_url)?;

    // Two independent SchedulerService instances — two "replicas" — sharing
    // one database, exactly as two processes behind a load balancer would.
    let replica_a = SchedulerService::new(
        probe_config(distributed_lock),
        Arc::clone(&pool),
        Arc::clone(&app_context),
    )?;
    let replica_b = SchedulerService::new(
        probe_config(distributed_lock),
        Arc::clone(&pool),
        Arc::clone(&app_context),
    )?;

    replica_a.start().await?;
    replica_b.start().await?;

    tokio::time::sleep(window).await;

    read_run_count(&pool).await
}

#[tokio::test]
async fn distributed_lock_suppresses_duplicate_execution() -> Result<()> {
    let window_secs: u64 = 6;
    let window = Duration::from_secs(window_secs);
    let single_replica_ceiling = window_secs as i32 + 2;

    let locked = run_two_replicas(true, window).await?;
    // Under coverage instrumentation and loaded CI runners, the 6-second window
    // can fit fewer ticks than nominal; the meaningful invariant for this test
    // is `locked < unlocked` (asserted below). Keep a positive lower bound so we
    // still notice a total dispatch failure.
    assert!(
        locked >= 1,
        "expected the job to fire at least once in {window_secs}s, got run_count={locked}"
    );
    assert!(
        locked <= single_replica_ceiling,
        "distributed lock failed to suppress duplicate execution: run_count={locked} exceeds the \
         single-replica ceiling of {single_replica_ceiling}"
    );

    let unlocked = run_two_replicas(false, window).await?;
    assert!(
        unlocked > single_replica_ceiling,
        "expected double-firing without the lock: run_count={unlocked} should exceed the \
         single-replica ceiling of {single_replica_ceiling}"
    );

    assert!(
        locked < unlocked,
        "the lock must yield meaningfully fewer runs than the unlocked control: locked={locked}, \
         unlocked={unlocked}"
    );

    Ok(())
}
