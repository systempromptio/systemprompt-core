//! Retention purges: the per-table deletes the scheduler's `database_cleanup`
//! job dispatches to, declared by the crate that owns each table.
//!
//! The job plans one pass per `RetentionTarget` with its window from
//! `profile.retention`; each pass deletes through the [`RetentionPurge`] an
//! admitted member contributed for that target's table, batch by batch. A
//! target with no contributed purge is not pruned; two purges for one table
//! refuse the run. A [`RetentionSweep`] runs once before the passes, for an
//! owner's own orphan settlement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_provider_contracts::{JobContext, ProviderResult};

/// One table's age-based delete. `#[async_trait]` keeps the trait
/// `dyn`-compatible.
#[async_trait]
pub trait RetentionPurge: Send + Sync {
    fn table(&self) -> &'static str;

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64>;
}

/// A pass an owner runs once per retention run, before the table passes.
/// `#[async_trait]` keeps the trait `dyn`-compatible.
#[async_trait]
pub trait RetentionSweep: Send + Sync {
    fn name(&self) -> &'static str;

    async fn sweep(&self, ctx: &JobContext, enforce: bool) -> ProviderResult<()>;
}
