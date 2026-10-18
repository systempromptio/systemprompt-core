//! Job draining the link-click projections: `analytics_event_link_facts`
//! (events an upgrade queued before their target url and external flag were
//! stored) and `analytics_link_click_rollup_hourly` (hours queued for a
//! rebuild). New events are stamped and rolled up by the logging schema's
//! statement triggers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use super::projection_drain::{ProjectionDrain, drain_projection};

const FACTS: ProjectionDrain = ProjectionDrain {
    projection: "analytics_event_link_facts",
    projector: "refresh_analytics_event_link_facts",
    batch: 1000,
    budget: Duration::from_secs(10),
};
const ROLLUP: ProjectionDrain = ProjectionDrain {
    projection: "analytics_link_click_rollup_hourly",
    projector: "refresh_analytics_link_click_rollup_hourly",
    batch: 24,
    budget: Duration::from_secs(10),
};

#[derive(Debug, Clone, Copy)]
pub struct AnalyticsEventLinkFactsRollupJob;

#[async_trait]
impl Job for AnalyticsEventLinkFactsRollupJob {
    fn name(&self) -> &'static str {
        "analytics_event_link_facts_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued link clicks into their stored target facts and the hourly page-to-target rollup"
    }

    fn schedule(&self) -> &'static str {
        "50 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("50 * * * * *")
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let facts = drain_projection(ctx, &FACTS).await?;
        let rollup = drain_projection(ctx, &ROLLUP).await?;
        let sum = |a: Option<u64>, b: Option<u64>| a.unwrap_or(0).saturating_add(b.unwrap_or(0));
        Ok(JobResult::success()
            .with_stats(
                sum(facts.items_processed, rollup.items_processed),
                sum(facts.items_failed, rollup.items_failed),
            )
            .with_duration(facts.duration_ms.saturating_add(rollup.duration_ms)))
    }
}

systemprompt_provider_contracts::submit_job!(&AnalyticsEventLinkFactsRollupJob);
