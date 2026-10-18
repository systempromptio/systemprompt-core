//! Job draining the `session_bot_type` projection: bot sessions an upgrade
//! queued before their taxonomy label was stored. New and reclassified
//! sessions are stamped by the users schema's statement triggers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use super::projection_drain::{ProjectionDrain, drain_projection};

const DRAIN: ProjectionDrain = ProjectionDrain {
    projection: "session_bot_type",
    projector: "refresh_session_bot_type",
    batch: 1000,
    budget: Duration::from_secs(20),
};

#[derive(Debug, Clone, Copy)]
pub struct SessionBotTypeRollupJob;

#[async_trait]
impl Job for SessionBotTypeRollupJob {
    fn name(&self) -> &'static str {
        "session_bot_type_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued bot sessions into their stored bot taxonomy label"
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

systemprompt_provider_contracts::submit_job!(&SessionBotTypeRollupJob);
