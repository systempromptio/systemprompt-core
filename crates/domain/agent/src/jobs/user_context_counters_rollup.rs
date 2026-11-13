//! Job draining the `user_context_counters` projection: contexts an upgrade
//! queued before their task count, message count and last message time were
//! stored. New writes keep the counters current in their own statement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use systemprompt_database::projection::{ProjectionDrain, drain_projection};

const DRAIN: ProjectionDrain = ProjectionDrain {
    projection: "user_context_counters",
    projector: "refresh_user_context_counters",
    batch: 500,
    budget: Duration::from_secs(20),
};

#[derive(Debug, Clone, Copy)]
pub struct UserContextCountersRollupJob;

#[async_trait]
impl Job for UserContextCountersRollupJob {
    fn name(&self) -> &'static str {
        "user_context_counters_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued contexts into their stored task count, message count and last message time"
    }

    fn schedule(&self) -> &'static str {
        "25 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("25 * * * * *")
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        drain_projection(ctx, &DRAIN).await
    }
}

systemprompt_traits::submit_job!(&UserContextCountersRollupJob);
