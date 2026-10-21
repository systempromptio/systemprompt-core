//! The batched delete loop the retention plan is executed with; the per-table
//! statements live in [`RetentionRepository`].
//!
//! Each table is deleted in batches of `BATCH_ROWS` through
//! [`run_batched`] so no statement takes a long lock and each statement-level
//! capture trigger writes one outbox row per batch instead of one per row. A
//! run stops early at `RUN_BUDGET` and picks up where it left off the next
//! night.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use systemprompt_traits::{BatchPlan, ProviderResult, run_batched};

use crate::repository::{RETENTION_BATCH_ROWS, RetentionRepository};

const RUN_BUDGET: Duration = Duration::from_mins(15);

#[derive(Debug, Clone)]
pub(super) struct RetentionPass {
    pub(super) table: &'static str,
    pub(super) days: u32,
    pub(super) deleted: u64,
    pub(super) complete: bool,
}

pub(super) async fn delete_in_batches(
    retention: &RetentionRepository,
    table: &'static str,
    days: u32,
    cutoff: DateTime<Utc>,
    started: Instant,
) -> ProviderResult<RetentionPass> {
    let plan = BatchPlan {
        deadline: started + RUN_BUDGET,
        full_batch: u64::try_from(RETENTION_BATCH_ROWS).unwrap_or(u64::MAX),
    };
    let run = run_batched(plan, move || retention.delete_batch(table, cutoff)).await?;
    Ok(RetentionPass {
        table,
        days,
        deleted: run.processed,
        complete: run.complete,
    })
}
