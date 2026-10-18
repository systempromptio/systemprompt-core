//! Inventory-registered test jobs used to drive dispatch outcome arms
//! (panic capture, failure recording, in-process overlap skip) that the
//! crate's built-in jobs never exercise.
//!
//! All jobs return `enabled() == false` so `JobSelection::All` (which filters
//! on the trait's `enabled`) never picks them up; dispatch and bootstrap run
//! them by explicit name regardless.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope};

pub const PANIC_JOB: &str = "sp_test_panic_job";
pub const FAILING_JOB: &str = "sp_test_failing_job";
pub const SLOW_JOB: &str = "sp_test_slow_job";
pub const EMPTY_SCHEDULE_JOB: &str = "sp_test_empty_schedule_job";
pub const NODE_JOB: &str = "sp_test_node_job";
pub const STAMP_FAIL_JOB: &str = "sp_test_stamp_fail_job";
pub const STAMP_PANIC_JOB: &str = "sp_test_stamp_panic_job";
pub const CANCELLABLE_CLUSTER_JOB: &str = "sp_test_cancellable_cluster_job";
pub const WORKING_JOB: &str = "sp_test_working_job";
pub const IDLE_JOB: &str = "sp_test_idle_job";
pub const BUDGETED_JOB: &str = "sp_test_budgeted_job";

pub static BUDGETED_JOB_SETTINGS: std::sync::Mutex<Vec<(String, String, String)>> =
    std::sync::Mutex::new(Vec::new());

pub static SLOW_JOB_STARTS: AtomicU64 = AtomicU64::new(0);
pub static NODE_JOB_RUNS: AtomicU64 = AtomicU64::new(0);
pub static STAMP_FAIL_JOB_RUNS: AtomicU64 = AtomicU64::new(0);
pub static CANCELLABLE_CLUSTER_JOB_RUNS: AtomicU64 = AtomicU64::new(0);
static CANCELLABLE_CLUSTER_JOB_STARTED: OnceLock<tokio::sync::Notify> = OnceLock::new();

pub fn cancellable_cluster_job_started() -> &'static tokio::sync::Notify {
    CANCELLABLE_CLUSTER_JOB_STARTED.get_or_init(tokio::sync::Notify::new)
}

struct CancellableClusterJob;

#[async_trait]
impl Job for CancellableClusterJob {
    fn name(&self) -> &'static str {
        CANCELLABLE_CLUSTER_JOB
    }
    fn schedule(&self) -> &'static str {
        ""
    }
    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        if CANCELLABLE_CLUSTER_JOB_RUNS.fetch_add(1, Ordering::SeqCst) == 0 {
            cancellable_cluster_job_started().notify_one();
            std::future::pending::<()>().await;
        }
        Ok(JobResult::success())
    }
}

struct WorkingJob;

#[async_trait]
impl Job for WorkingJob {
    fn name(&self) -> &'static str {
        WORKING_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        Ok(JobResult::success().with_stats(1, 0))
    }
}

struct IdleJob;

#[async_trait]
impl Job for IdleJob {
    fn name(&self) -> &'static str {
        IDLE_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        Ok(JobResult::success().with_stats(0, 0))
    }
}

struct PanicJob;

#[async_trait]
impl Job for PanicJob {
    fn name(&self) -> &'static str {
        PANIC_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        panic!("deliberate test panic payload");
    }
}

struct FailingJob;

#[async_trait]
impl Job for FailingJob {
    fn name(&self) -> &'static str {
        FAILING_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        Ok(JobResult::failure("deliberate test failure"))
    }
}

struct SlowJob;

#[async_trait]
impl Job for SlowJob {
    fn name(&self) -> &'static str {
        SLOW_JOB
    }

    fn schedule(&self) -> &'static str {
        "* * * * * *"
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        SLOW_JOB_STARTS.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        Ok(JobResult::success())
    }
}

struct EmptyScheduleJob;

#[async_trait]
impl Job for EmptyScheduleJob {
    fn name(&self) -> &'static str {
        EMPTY_SCHEDULE_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

struct NodeJob;

#[async_trait]
impl Job for NodeJob {
    fn name(&self) -> &'static str {
        NODE_JOB
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
        NODE_JOB_RUNS.fetch_add(1, Ordering::SeqCst);
        Ok(JobResult::success())
    }
}

struct StampFailJob;

#[async_trait]
impl Job for StampFailJob {
    fn name(&self) -> &'static str {
        STAMP_FAIL_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        STAMP_FAIL_JOB_RUNS.fetch_add(1, Ordering::SeqCst);
        Ok(JobResult::failure("stamp job failed on purpose"))
    }
}

struct StampPanicJob;

#[async_trait]
impl Job for StampPanicJob {
    fn name(&self) -> &'static str {
        STAMP_PANIC_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        panic!("stamp job panic payload");
    }
}

systemprompt_provider_contracts::submit_job!(&NodeJob);
systemprompt_provider_contracts::submit_job!(&StampFailJob);
systemprompt_provider_contracts::submit_job!(&StampPanicJob);
systemprompt_provider_contracts::submit_job!(&PanicJob);
systemprompt_provider_contracts::submit_job!(&FailingJob);
systemprompt_provider_contracts::submit_job!(&SlowJob);
systemprompt_provider_contracts::submit_job!(&EmptyScheduleJob);
systemprompt_provider_contracts::submit_job!(&CancellableClusterJob);
systemprompt_provider_contracts::submit_job!(&WorkingJob);
systemprompt_provider_contracts::submit_job!(&IdleJob);
systemprompt_provider_contracts::submit_job!(&BudgetedJob);

struct BudgetedJob;

#[async_trait]
impl Job for BudgetedJob {
    fn name(&self) -> &'static str {
        BUDGETED_JOB
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn enabled(&self) -> bool {
        false
    }

    fn db_budget(&self) -> Option<systemprompt_traits::DbBudget> {
        Some(
            systemprompt_traits::DbBudget::new(std::time::Duration::from_secs(7))
                .with_lock_timeout(std::time::Duration::from_secs(3))
                .with_work_mem_kb(2048),
        )
    }

    async fn execute(
        &self,
        ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        let db = ctx
            .get::<systemprompt_database::BudgetedDb>()
            .expect("dispatch injects a BudgetedDb");
        let mut tx = db.begin().await.expect("budgeted transaction");
        let row: (String, String, String) = sqlx::query_as(
            "SELECT current_setting('statement_timeout'), current_setting('lock_timeout'), \
             current_setting('work_mem')",
        )
        .fetch_one(&mut *tx)
        .await
        .expect("read budget settings");
        tx.rollback().await.expect("rollback");
        BUDGETED_JOB_SETTINGS.lock().expect("lock").push(row);
        Ok(JobResult::success().with_stats(1, 0))
    }
}
