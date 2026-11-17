//! `scheduled_jobs` rows across nodes: a scheduler start never deletes a row,
//! keeps the operator's `enabled` flag, and never claims or runs a job it does
//! not register; a row disabled by the operator does not tick; and
//! [`JobPruneService`] lists the rows no admitted job names and
//! deletes exactly the rows it is handed. These tests write `scheduled_jobs`
//! and join the serialized `scheduler-jobs-db` nextest group.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::JobName;
use systemprompt_manifest::services::UnlistedJobs;
use systemprompt_scheduler::{
    JobConfig, JobPruneService, JobRepository, SchedulerConfig, SchedulerRepository,
    SchedulerService, SchedulerStartup,
};
use systemprompt_test_fixtures::{test_database_url, test_db_pool};
use systemprompt_traits::{Job, JobContext, JobResult};

const NODE_A_JOB: &str = "sp_test_rows_node_a_job";
const NODE_B_JOB: &str = "sp_test_rows_node_b_job";
const NODE_A_TICK: &str = "sp_test_rows_node_a_tick";
const NODE_B_TICK: &str = "sp_test_rows_node_b_tick";
const DISABLED_TICK: &str = "sp_test_rows_disabled_tick";
const FAR_SCHEDULE: &str = "0 0 4 1 1 *";
const EVERY_SECOND: &str = "* * * * * *";

static NODE_A_TICKS: AtomicU64 = AtomicU64::new(0);
static NODE_B_TICKS: AtomicU64 = AtomicU64::new(0);
static DISABLED_TICKS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct RowsJob {
    name: &'static str,
    schedule: &'static str,
    runs: Option<&'static AtomicU64>,
}

#[async_trait]
impl Job for RowsJob {
    fn name(&self) -> &'static str {
        self.name
    }

    fn schedule(&self) -> &'static str {
        self.schedule
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        if let Some(runs) = self.runs {
            runs.fetch_add(1, Ordering::SeqCst);
        }
        Ok(JobResult::success().with_stats(1, 0))
    }
}

static NODE_A: RowsJob = RowsJob {
    name: NODE_A_JOB,
    schedule: FAR_SCHEDULE,
    runs: None,
};
static NODE_B: RowsJob = RowsJob {
    name: NODE_B_JOB,
    schedule: FAR_SCHEDULE,
    runs: None,
};
static NODE_A_TICKER: RowsJob = RowsJob {
    name: NODE_A_TICK,
    schedule: EVERY_SECOND,
    runs: Some(&NODE_A_TICKS),
};
static NODE_B_TICKER: RowsJob = RowsJob {
    name: NODE_B_TICK,
    schedule: EVERY_SECOND,
    runs: Some(&NODE_B_TICKS),
};
static DISABLED_TICKER: RowsJob = RowsJob {
    name: DISABLED_TICK,
    schedule: EVERY_SECOND,
    runs: Some(&DISABLED_TICKS),
};

pub(crate) fn contribute(c: &mut systemprompt_extension::Contributions<'_>) {
    c.job(NODE_A)
        .job(NODE_B)
        .job(NODE_A_TICKER)
        .job(NODE_B_TICKER)
        .job(DISABLED_TICKER);
}

fn config(jobs: &[&str], schedule: &str) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: jobs
            .iter()
            .map(|name| JobConfig::new(JobName::new(*name)).with_schedule(schedule))
            .collect(),
        bootstrap_jobs: Vec::new(),
        distributed_lock: false,
        unlisted_jobs: UnlistedJobs::Warn,
    }
}

async fn start(pool: &DbPool, config: SchedulerConfig) -> SchedulerStartup {
    let (app_ctx, schedule) = crate::test_extension::scheduled_app(&test_database_url()).await;
    SchedulerService::new(config, pool, app_ctx, Arc::clone(&schedule))
        .start()
        .await
        .expect("scheduler starts")
}

async fn stop(startup: SchedulerStartup) {
    if let Some(handle) = startup.handle {
        handle.shutdown().await.expect("shutdown");
    }
}

async fn reset_rows(pool: &DbPool, names: &[&str]) {
    for name in names {
        sqlx::query!("DELETE FROM scheduled_jobs WHERE job_name = $1", *name)
            .execute(&*pool.write_pool())
            .await
            .expect("clear test row");
    }
}

async fn age_row(pool: &DbPool, name: &str) {
    sqlx::query!(
        "UPDATE scheduled_jobs SET updated_at = NOW() - INTERVAL '30 days' WHERE job_name = $1",
        name
    )
    .execute(&*pool.write_pool())
    .await
    .expect("age test row");
}

async fn wait_for_tick(counter: &AtomicU64, before: u64) -> bool {
    tokio::time::timeout(Duration::from_secs(15), async {
        while counter.load(Ordering::SeqCst) == before {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok()
}

mod two_nodes_db {
    use super::*;

    #[tokio::test]
    async fn nodes_with_different_job_sets_keep_each_others_rows_and_flags() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        reset_rows(&pool, &[NODE_A_JOB, NODE_B_JOB]).await;

        repo.upsert_job(&JobName::new(NODE_B_JOB), FAR_SCHEDULE, true)
            .await
            .expect("seed node B row");
        JobRepository::new(&pool)
            .set_enabled(&JobName::new(NODE_B_JOB), false)
            .await
            .expect("operator disables node B's job");
        age_row(&pool, NODE_B_JOB).await;

        stop(start(&pool, config(&[NODE_A_JOB], FAR_SCHEDULE)).await).await;
        age_row(&pool, NODE_A_JOB).await;
        stop(start(&pool, config(&[NODE_B_JOB], FAR_SCHEDULE)).await).await;

        let a = repo
            .find_job(&JobName::new(NODE_A_JOB))
            .await
            .expect("find")
            .expect("node B's start must keep node A's row");
        let b = repo
            .find_job(&JobName::new(NODE_B_JOB))
            .await
            .expect("find")
            .expect("node A's start must keep node B's row");
        assert!(a.enabled, "node A's row keeps enabled = true");
        assert!(
            !b.enabled,
            "node B's own start must keep the operator's enabled = false"
        );
    }

    #[tokio::test]
    async fn a_node_never_runs_a_job_it_does_not_register() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        reset_rows(&pool, &[NODE_A_TICK, NODE_B_TICK]).await;
        repo.upsert_job(&JobName::new(NODE_B_TICK), EVERY_SECOND, true)
            .await
            .expect("seed node B's every-second row");

        let a_before = NODE_A_TICKS.load(Ordering::SeqCst);
        let b_before = NODE_B_TICKS.load(Ordering::SeqCst);
        let startup = start(&pool, config(&[NODE_A_TICK], EVERY_SECOND)).await;
        let fired = wait_for_tick(&NODE_A_TICKS, a_before).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        stop(startup).await;

        assert!(fired, "the registered job must tick (positive control)");
        assert_eq!(
            NODE_B_TICKS.load(Ordering::SeqCst),
            b_before,
            "a job this node does not register must never run"
        );
        let b = repo
            .find_job(&JobName::new(NODE_B_TICK))
            .await
            .expect("find")
            .expect("row kept");
        assert!(
            b.last_run.is_none(),
            "the unregistered row is never claimed"
        );
        assert!(b.last_instance_id.is_none());
    }

    #[tokio::test]
    async fn a_row_the_operator_disabled_does_not_tick() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        reset_rows(&pool, &[NODE_A_TICK, DISABLED_TICK]).await;
        repo.upsert_job(&JobName::new(DISABLED_TICK), EVERY_SECOND, true)
            .await
            .expect("seed row");
        JobRepository::new(&pool)
            .set_enabled(&JobName::new(DISABLED_TICK), false)
            .await
            .expect("operator disables");

        let a_before = NODE_A_TICKS.load(Ordering::SeqCst);
        let disabled_before = DISABLED_TICKS.load(Ordering::SeqCst);
        let startup = start(&pool, config(&[NODE_A_TICK, DISABLED_TICK], EVERY_SECOND)).await;
        assert_eq!(startup.scheduled, 2, "both jobs are on the cron loop");
        let fired = wait_for_tick(&NODE_A_TICKS, a_before).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        stop(startup).await;

        assert!(fired, "the enabled sibling must tick (positive control)");
        assert_eq!(
            DISABLED_TICKS.load(Ordering::SeqCst),
            disabled_before,
            "a job whose row is enabled = false must not run on a tick"
        );
    }
}

mod prune_db {
    use super::*;

    fn unregistered_name() -> JobName {
        JobName::new(format!(
            "sp_test_rows_retired_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    async fn service(pool: &DbPool) -> JobPruneService {
        let schedule =
            crate::test_extension::schedule_of(&crate::test_extension::admitted_with_test_jobs());
        JobPruneService::new(JobRepository::new(pool), &schedule)
    }

    #[tokio::test]
    async fn listing_reports_unregistered_rows_and_deletes_nothing() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let retired = unregistered_name();
        repo.upsert_job(&retired, FAR_SCHEDULE, true)
            .await
            .expect("seed retired");
        repo.upsert_job(&JobName::new(NODE_A_JOB), FAR_SCHEDULE, true)
            .await
            .expect("seed registered");

        let listed = service(&pool).await.unregistered().await.expect("list");
        let names: Vec<&str> = listed.iter().map(|row| row.job_name.as_str()).collect();

        assert!(
            names.contains(&retired.as_str()),
            "the unregistered row is listed"
        );
        assert!(
            !names.contains(&NODE_A_JOB),
            "a registered row is never listed"
        );
        assert!(
            repo.find_job(&retired).await.expect("find").is_some(),
            "listing deletes nothing"
        );
        reset_rows(&pool, &[retired.as_str()]).await;
    }

    #[tokio::test]
    async fn prune_deletes_exactly_the_unregistered_rows_it_is_given() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let retired = unregistered_name();
        let registered = JobName::new(NODE_A_JOB);
        repo.upsert_job(&retired, FAR_SCHEDULE, true)
            .await
            .expect("seed retired");
        repo.upsert_job(&registered, FAR_SCHEDULE, true)
            .await
            .expect("seed registered");

        let deleted = service(&pool)
            .await
            .delete(&[retired.clone(), registered.clone()])
            .await
            .expect("prune");

        assert_eq!(deleted, 1, "only the unregistered row is deleted");
        assert!(repo.find_job(&retired).await.expect("find").is_none());
        assert!(
            repo.find_job(&registered).await.expect("find").is_some(),
            "a registered row survives even when named"
        );
    }

    #[tokio::test]
    async fn a_job_of_a_member_this_composition_leaves_out_is_listed() {
        let pool = test_db_pool().await;
        let repo = SchedulerRepository::new(&pool);
        let owned = JobName::new(NODE_B_JOB);
        repo.upsert_job(&owned, FAR_SCHEDULE, true)
            .await
            .expect("seed row");

        let admitted = service(&pool).await.unregistered().await.expect("list");
        assert!(
            admitted.iter().all(|row| row.job_name != owned),
            "a job of an admitted member is never listed"
        );

        let without = JobPruneService::new(
            JobRepository::new(&pool),
            &crate::test_extension::core_schedule(),
        );
        let listed = without.unregistered().await.expect("list");
        assert!(
            listed.iter().any(|row| row.job_name == owned),
            "a job whose member is not admitted is listed"
        );
        reset_rows(&pool, &[NODE_B_JOB]).await;
    }
}
