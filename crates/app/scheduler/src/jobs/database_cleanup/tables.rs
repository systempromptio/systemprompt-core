//! The owner repositories the retention plan dispatches to, and the batched
//! delete loop that drives them.
//!
//! Each target is deleted in batches of [`BATCH_ROWS`] through
//! [`run_batched`] so no statement takes a long lock and each statement-level
//! capture trigger writes one outbox row per batch instead of one per row. A
//! run stops early at `RUN_BUDGET` and picks up where it left off the next
//! night.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use systemprompt_ai::repository::{
    AiQuotaBucketRepository, AiRequestPayloadRepository, AiRequestRepository,
};
use systemprompt_analytics::AnalyticsRetention;
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::DurableOutbox;
use systemprompt_extension::RetentionHolds;
use systemprompt_governance::GovernanceDecisionRetention;
use systemprompt_identifiers::InstanceId;
use systemprompt_logging::{AnalyticsRepository, LoggingRepository};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_provider_contracts::{BatchPlan, run_batched};
use systemprompt_traits::ProviderResult;
use tracing::info;

use super::RetentionTarget;
use super::holds::{HoldOutcome, report};
use crate::error::SchedulerResult;

pub(super) const BATCH_ROWS: i64 = 5000;
const RUN_BUDGET: Duration = Duration::from_mins(15);

#[derive(Debug, Clone)]
pub(super) struct RetentionOwners {
    pub(super) logs: LoggingRepository,
    pub(super) analytics_events: AnalyticsRepository,
    pub(super) requests: AiRequestRepository,
    pub(super) payloads: AiRequestPayloadRepository,
    pub(super) quota_buckets: AiQuotaBucketRepository,
    pub(super) tool_executions: ToolUsageRepository,
    pub(super) outbox: DurableOutbox,
    pub(super) decisions: GovernanceDecisionRetention,
    pub(super) analytics_pii: AnalyticsRetention,
}

impl RetentionOwners {
    pub(super) fn new(db: &DbPool, instance_id: InstanceId) -> Self {
        Self {
            logs: LoggingRepository::new(db),
            analytics_events: AnalyticsRepository::new(db),
            requests: AiRequestRepository::new(db),
            payloads: AiRequestPayloadRepository::new(db),
            quota_buckets: AiQuotaBucketRepository::new(db),
            tool_executions: ToolUsageRepository::new(db),
            outbox: DurableOutbox::new(db.write_pool().as_ref().clone(), instance_id),
            decisions: GovernanceDecisionRetention::new(db),
            analytics_pii: AnalyticsRetention::new(db),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct RetentionPass {
    pub(super) table: &'static str,
    pub(super) deleted: u64,
    pub(super) complete: bool,
    pub(super) hold: HoldOutcome,
}

pub(super) struct PassRunner<'a> {
    pub(super) owners: &'a RetentionOwners,
    pub(super) holds: &'a RetentionHolds,
    pub(super) enforce: bool,
    pub(super) started: Instant,
}

impl PassRunner<'_> {
    pub(super) async fn run(
        &self,
        target: RetentionTarget,
        days: u32,
    ) -> ProviderResult<RetentionPass> {
        let age_cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        let effective = self.holds.resolve(target, age_cutoff).await;
        let hold = report(target, days, age_cutoff, &effective);
        let (deleted, complete) = match effective.cutoff() {
            None => (0, false),
            Some(cutoff) if self.enforce => self.delete_in_batches(target, cutoff).await?,
            Some(_) => {
                info!(
                    table = target.table(),
                    retention_days = days,
                    "enforce disabled: rows past the window were not deleted"
                );
                (0, true)
            },
        };
        info!(
            table = target.table(),
            deleted,
            retention_days = days,
            complete,
            effective_cutoff = ?effective.cutoff(),
            hold = ?hold,
            "Retention pass"
        );
        Ok(RetentionPass {
            table: target.table(),
            deleted,
            complete,
            hold,
        })
    }

    async fn delete_in_batches(
        &self,
        target: RetentionTarget,
        cutoff: DateTime<Utc>,
    ) -> ProviderResult<(u64, bool)> {
        let plan = BatchPlan {
            deadline: self.started + RUN_BUDGET,
            full_batch: u64::try_from(BATCH_ROWS).unwrap_or(u64::MAX),
        };
        let owners = self.owners;
        let run = run_batched(plan, move || purge(target, owners, cutoff)).await?;
        Ok((run.processed, run.complete))
    }
}

async fn purge(
    target: RetentionTarget,
    owners: &RetentionOwners,
    cutoff: DateTime<Utc>,
) -> SchedulerResult<u64> {
    let deleted = match target {
        RetentionTarget::Logs => owners.logs.purge_before(cutoff, BATCH_ROWS).await?,
        RetentionTarget::AnalyticsEvents => {
            owners
                .analytics_events
                .purge_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::AiRequestMessages => {
            owners
                .requests
                .purge_messages_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::McpToolExecutions => {
            owners
                .tool_executions
                .purge_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::EventOutbox => {
            owners
                .outbox
                .purge_processed_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::AiRequestPayloads => {
            owners
                .payloads
                .release_bodies_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::GovernanceDecisions => {
            owners.decisions.purge_before(cutoff, BATCH_ROWS).await?
        },
        RetentionTarget::AiQuotaBuckets => {
            owners
                .quota_buckets
                .purge_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::EngagementEvents => {
            owners
                .analytics_pii
                .purge_engagement_events_before(cutoff, BATCH_ROWS)
                .await?
        },
        RetentionTarget::FingerprintReputation => {
            owners
                .analytics_pii
                .purge_fingerprints_before(cutoff, BATCH_ROWS)
                .await?
        },
    };
    Ok(deleted)
}
