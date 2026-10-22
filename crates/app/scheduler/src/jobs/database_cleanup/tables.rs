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
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::DurableOutbox;
use systemprompt_governance::GovernanceDecisionRetention;
use systemprompt_identifiers::InstanceId;
use systemprompt_logging::{AnalyticsRepository, LoggingRepository};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_provider_contracts::{BatchPlan, run_batched};
use systemprompt_traits::ProviderResult;

use super::RetentionTarget;

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
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct RetentionPass {
    pub(super) table: &'static str,
    pub(super) days: u32,
    pub(super) deleted: u64,
    pub(super) complete: bool,
}

pub(super) async fn delete_in_batches(
    owners: &RetentionOwners,
    target: RetentionTarget,
    days: u32,
    cutoff: DateTime<Utc>,
    started: Instant,
) -> ProviderResult<RetentionPass> {
    let plan = BatchPlan {
        deadline: started + RUN_BUDGET,
        full_batch: u64::try_from(BATCH_ROWS).unwrap_or(u64::MAX),
    };
    let run = run_batched(plan, move || target.purge(owners, cutoff)).await?;
    Ok(RetentionPass {
        table: target.table(),
        days,
        deleted: run.processed,
        complete: run.complete,
    })
}
