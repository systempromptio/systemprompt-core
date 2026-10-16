//! Job draining the `log_rollup_hourly` projection: log hours a delete (or
//! an upgrade) queued for a recount. Inserts add to their hour in their own
//! statement, so this only works off deletes and history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use super::projection_drain::{ProjectionDrain, drain_projection};

const DRAIN: ProjectionDrain = ProjectionDrain {
    projection: "log_rollup_hourly",
    projector: "refresh_log_rollup_hourly",
    batch: 500,
    budget: Duration::from_secs(20),
};

#[derive(Debug, Clone, Copy)]
pub struct LogSummaryRollupJob;

#[async_trait]
impl Job for LogSummaryRollupJob {
    fn name(&self) -> &'static str {
        "log_summary_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued log hours into the hourly rollup the log summary reads"
    }

    fn schedule(&self) -> &'static str {
        "35 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("35 * * * * *")
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        drain_projection(ctx, &DRAIN).await
    }
}

systemprompt_provider_contracts::submit_job!(&LogSummaryRollupJob);
