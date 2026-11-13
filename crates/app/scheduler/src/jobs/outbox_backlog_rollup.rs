//! Drains the `event_outbox_backlog` projection — the consumers an upgrade
//! queued for a recount of their pending outbox rows — then compacts the
//! delta rows the outbox's statement triggers append to one row per
//! consumer, so a health read sums a handful of rows rather than a minute
//! of appends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::ProviderError;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use systemprompt_database::projection::{ProjectionDrain, drain_projection};

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
        "Recounts queued outbox consumers and compacts their pending-row deltas"
    }

    fn schedule(&self) -> &'static str {
        "40 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("40 * * * * *")
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let drained = drain_projection(ctx, &DRAIN).await?;
        let db_pool = Arc::clone(ctx.get::<DbPool>()?);
        sqlx::query_scalar!(r#"SELECT compact_event_outbox_backlog() AS "compacted!""#)
            .fetch_one(db_pool.write_pool().as_ref())
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        Ok(drained)
    }
}

systemprompt_provider_contracts::submit_job!("scheduler", &OutboxBacklogRollupJob);
