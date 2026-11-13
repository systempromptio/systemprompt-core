//! The scheduler's cross-replica single-execution guarantee.
//!
//! Two `SchedulerService` replicas share one database and tick the same job
//! every second. The guarantee rests on the Postgres advisory lock the cluster
//! claim takes when `SchedulerConfig.distributed_lock` is true, so each test
//! holds that lock from a peer connection and observes the replicas against a
//! known lock state instead of counting ticks:
//!
//! * with the lock enabled and held by the peer, no tick runs the job — the run
//!   count stays exactly zero — and once the peer releases, the job runs;
//! * with the lock disabled, the peer's lock is ignored and the job runs.
//!
//! Every replica is shut down before the test returns. DB-backed: needs a
//! reachable Postgres (`DATABASE_URL`).

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, ensure};
use async_trait::async_trait;
use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use systemprompt_database::DbPool;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerHandle, SchedulerService};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

const LOCKED_JOB: &str = "test_distributed_lock_probe_locked";
const UNLOCKED_JOB: &str = "test_distributed_lock_probe_unlocked";
const HELD_TICKS: Duration = Duration::from_secs(3);
const RUN_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
struct LockedProbeJob;

#[async_trait]
impl Job for LockedProbeJob {
    fn name(&self) -> &'static str {
        LOCKED_JOB
    }

    fn description(&self) -> &'static str {
        "Test-only job exercising the scheduler distributed advisory lock"
    }

    fn schedule(&self) -> &'static str {
        "* * * * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

#[derive(Debug, Clone, Copy)]
struct UnlockedProbeJob;

#[async_trait]
impl Job for UnlockedProbeJob {
    fn name(&self) -> &'static str {
        UNLOCKED_JOB
    }

    fn description(&self) -> &'static str {
        "Test-only job running with the scheduler distributed lock disabled"
    }

    fn schedule(&self) -> &'static str {
        "* * * * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

systemprompt_traits::submit_job!("scheduler_test_jobs", &LockedProbeJob);
systemprompt_traits::submit_job!("scheduler_test_jobs", &UnlockedProbeJob);

fn probe_config(job: &str, distributed_lock: bool) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: vec![JobConfig::new(JobName::new(job)).with_schedule("* * * * * *")],
        bootstrap_jobs: vec![],
        distributed_lock,
        unlisted_jobs: systemprompt_manifest::services::UnlistedJobs::Warn,
    }
}

async fn reset_job_row(pool: &DbPool, job: &str) -> Result<()> {
    sqlx::query("DELETE FROM scheduled_jobs WHERE job_name = $1")
        .bind(job)
        .execute(pool.write_pool().as_ref())
        .await?;
    Ok(())
}

async fn read_run_count(pool: &DbPool, job: &str) -> Result<i32> {
    let count: Option<i32> =
        sqlx::query_scalar("SELECT run_count FROM scheduled_jobs WHERE job_name = $1")
            .bind(job)
            .fetch_optional(pool.write_pool().as_ref())
            .await?;
    Ok(count.unwrap_or(0))
}

async fn hold_job_lock(pool: &DbPool, job: &str) -> Result<PoolConnection<Postgres>> {
    let mut peer = pool.write_pool().acquire().await?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext($1)::bigint)")
        .bind(job)
        .fetch_one(peer.as_mut())
        .await?;
    ensure!(acquired, "the peer must take the job lock first");
    Ok(peer)
}

async fn release_job_lock(mut peer: PoolConnection<Postgres>, job: &str) -> Result<()> {
    let released: bool = sqlx::query_scalar("SELECT pg_advisory_unlock(hashtext($1)::bigint)")
        .bind(job)
        .fetch_one(peer.as_mut())
        .await?;
    ensure!(released, "the peer must still hold the job lock");
    Ok(())
}

async fn start_replicas(job: &str, distributed_lock: bool) -> Result<Vec<SchedulerHandle>> {
    let database_url = test_database_url();
    let pool = test_db_pool().await;
    let app_context = test_app_context(&pool, &database_url);
    let mut handles = Vec::with_capacity(2);
    for _ in 0..2 {
        let startup = SchedulerService::new(
            probe_config(job, distributed_lock),
            Arc::clone(&pool),
            Arc::clone(&app_context),
        )
        .start()
        .await?;
        ensure!(startup.scheduled == 1, "the probe job must be scheduled");
        handles.push(startup.handle.expect("an enabled scheduler has a handle"));
    }
    Ok(handles)
}

async fn shutdown(handles: Vec<SchedulerHandle>) -> Result<()> {
    for handle in handles {
        handle.shutdown().await?;
    }
    Ok(())
}

async fn wait_for_a_run(pool: &DbPool, job: &str) -> Result<i32> {
    let deadline = tokio::time::Instant::now() + RUN_DEADLINE;
    loop {
        let count = read_run_count(pool, job).await?;
        if count >= 1 || tokio::time::Instant::now() >= deadline {
            return Ok(count);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn a_held_job_lock_stops_every_replica_until_it_is_released() -> Result<()> {
    let pool = test_db_pool().await;
    reset_job_row(&pool, LOCKED_JOB).await?;
    let peer = hold_job_lock(&pool, LOCKED_JOB).await?;
    let replicas = start_replicas(LOCKED_JOB, true).await?;

    tokio::time::sleep(HELD_TICKS).await;
    let while_held = read_run_count(&pool, LOCKED_JOB).await?;
    release_job_lock(peer, LOCKED_JOB).await?;
    let after_release = wait_for_a_run(&pool, LOCKED_JOB).await?;
    shutdown(replicas).await?;
    reset_job_row(&pool, LOCKED_JOB).await?;

    assert_eq!(
        while_held, 0,
        "no replica may run the job while another session holds its lock"
    );
    assert!(
        after_release >= 1,
        "a replica must run the job once the lock is free, got run_count={after_release}"
    );
    Ok(())
}

#[tokio::test]
async fn without_the_distributed_lock_replicas_ignore_a_held_job_lock() -> Result<()> {
    let pool = test_db_pool().await;
    reset_job_row(&pool, UNLOCKED_JOB).await?;
    let peer = hold_job_lock(&pool, UNLOCKED_JOB).await?;
    let replicas = start_replicas(UNLOCKED_JOB, false).await?;

    let runs = wait_for_a_run(&pool, UNLOCKED_JOB).await?;
    release_job_lock(peer, UNLOCKED_JOB).await?;
    shutdown(replicas).await?;
    reset_job_row(&pool, UNLOCKED_JOB).await?;

    assert!(
        runs >= 1,
        "with the lock disabled the job runs regardless of the peer, got run_count={runs}"
    );
    Ok(())
}
