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

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use systemprompt_extension::RetentionHolds;
use systemprompt_provider_contracts::{BatchPlan, run_batched};
use systemprompt_traits::{JobContext, ProviderResult, RetentionPurge, registered_retention_purges};
use tracing::info;

use super::RetentionTarget;
use super::holds::{HoldOutcome, report};
use crate::error::{SchedulerError, SchedulerResult};

pub(super) const BATCH_ROWS: i64 = 5000;
const RUN_BUDGET: Duration = Duration::from_mins(15);

/// The registered purge of every target table, one per table.
pub(super) struct RetentionOwners {
    purges: HashMap<&'static str, &'static dyn RetentionPurge>,
}

impl RetentionOwners {
    pub(super) fn discover() -> SchedulerResult<Self> {
        let mut purges: HashMap<&'static str, &'static dyn RetentionPurge> = HashMap::new();
        for purge in registered_retention_purges() {
            if purges.insert(purge.table(), purge).is_some() {
                return Err(SchedulerError::DuplicateRetentionPurge {
                    table: purge.table(),
                });
            }
        }
        Ok(Self { purges })
    }

    pub(super) fn owns(&self, target: RetentionTarget) -> bool {
        self.purges.contains_key(target.table())
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
    pub(super) ctx: &'a JobContext,
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
        let Some(purge) = self.owners.purges.get(target.table()).copied() else {
            return Ok((0, true));
        };
        let ctx = self.ctx;
        let run = run_batched(plan, move || purge.purge_before(ctx, cutoff, BATCH_ROWS)).await?;
        Ok((run.processed, run.complete))
    }
}
