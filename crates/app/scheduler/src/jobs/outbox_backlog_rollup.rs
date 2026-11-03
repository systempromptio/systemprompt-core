//! Drains the `event_outbox_backlog` projection: the consumers an upgrade
//! queued for a recount of their pending outbox rows. New rows keep the
//! counter current through the outbox's statement triggers; this job only
//! settles what was queued.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use super::projection_drain::{ProjectionDrain, drain_projection};

const DRAIN: ProjectionDrain = ProjectionDrain {
    projection: "event_outbox_backlog",
    projector: "refresh_event_outbox_backlog",
    batch: 64,
    budget: Duration::from_secs(10),
};

#[derive(Debug, Clone, Copy)]
pub struct OutboxBacklogRollupJob;

#[async_trait]
impl Job for OutboxBacklogRollupJob {
    fn name(&self) -> &'static str {
        "outbox_backlog_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued outbox consumers into their stored pending-row counter"
    }

    fn schedule(&self) -> &'static str {
        "40 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("40 * * * * *")
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        drain_projection(ctx, &DRAIN).await
    }
}

systemprompt_provider_contracts::submit_job!(&OutboxBacklogRollupJob);
