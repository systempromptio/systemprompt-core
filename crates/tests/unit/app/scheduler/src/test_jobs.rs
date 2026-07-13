//! Inventory-registered test jobs used to drive dispatch outcome arms
//! (panic capture, failure recording, in-process overlap skip) that the
//! crate's built-in jobs never exercise.
//!
//! All jobs return `enabled() == false` so `JobSelection::All` (which filters
//! on the trait's `enabled`) never picks them up; dispatch and bootstrap run
//! them by explicit name regardless.

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult};

pub const PANIC_JOB: &str = "sp_test_panic_job";
pub const FAILING_JOB: &str = "sp_test_failing_job";
pub const SLOW_JOB: &str = "sp_test_slow_job";
pub const EMPTY_SCHEDULE_JOB: &str = "sp_test_empty_schedule_job";

pub static SLOW_JOB_STARTS: AtomicU64 = AtomicU64::new(0);

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

systemprompt_provider_contracts::submit_job!(&PanicJob);
systemprompt_provider_contracts::submit_job!(&FailingJob);
systemprompt_provider_contracts::submit_job!(&SlowJob);
systemprompt_provider_contracts::submit_job!(&EmptyScheduleJob);
