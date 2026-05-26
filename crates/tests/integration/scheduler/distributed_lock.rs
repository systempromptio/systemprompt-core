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
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerService};
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

/// Job name used by both replicas in this test. Unique enough not to collide
/// with the built-in jobs discovered via `inventory`.
const TEST_JOB_NAME: &str = "test_distributed_lock_probe";

/// A trivial job whose only purpose is to be scheduled. The scheduler
/// increments `scheduled_jobs.run_count` for every execution it dispatches,
/// so the job body itself does not need to touch the database — the run
/// count is the observable signal.
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

/// Scheduler config with exactly the probe job, on a one-second cron.
fn probe_config(distributed_lock: bool) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: vec![JobConfig::new(TEST_JOB_NAME, fixture_user_id()).with_schedule("* * * * * *")],
        bootstrap_jobs: vec![],
        distributed_lock,
    }
}

/// Reset the probe job's `scheduled_jobs` row so each test starts from a
/// known baseline regardless of prior runs.
async fn reset_job_row(pool: &DbPool) -> Result<()> {
    sqlx::query("DELETE FROM scheduled_jobs WHERE job_name = $1")
        .bind(TEST_JOB_NAME)
        .execute(pool.write_pool_arc()?.as_ref())
        .await?;
    Ok(())
}

/// Read the current `run_count` for the probe job, or 0 if the row is absent.
async fn read_run_count(pool: &DbPool) -> Result<i32> {
    let count: Option<i32> =
        sqlx::query_scalar("SELECT run_count FROM scheduled_jobs WHERE job_name = $1")
            .bind(TEST_JOB_NAME)
            .fetch_optional(pool.pool_arc()?.as_ref())
            .await?;
    Ok(count.unwrap_or(0))
}

/// Run two scheduler replicas against one database for `window`, then return
/// the observed `run_count` for the probe job.
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

/// Regression test for the cross-replica single-execution guarantee.
///
/// Both phases share the one `scheduled_jobs` probe row, so they must run
/// sequentially in a single test — two `#[tokio::test]` functions would race
/// on that row under the default parallel test harness.
///
/// Phase 1 (lock enabled): two replicas sharing a database run the job ONCE
/// per tick. Over a `W`-second window we expect roughly `W` one-second ticks;
/// cron alignment puts the true count in `W - 1 ..= W + 1`. The defining
/// property of the fix is that the count stays near the single-replica rate
/// and does not approach `2 * W`.
///
/// Phase 2 (negative control, lock disabled): the same setup double-fires —
/// both replicas dispatch every tick — documenting exactly what the lock
/// prevents.
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
