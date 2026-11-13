//! Retention purges: the per-table deletes the scheduler's `database_cleanup`
//! job dispatches to, declared by the crate that owns each table.
//!
//! The job plans one pass per `RetentionTarget` with its window from
//! `profile.retention`; each pass deletes through the [`RetentionPurge`]
//! registered for that target's table, batch by batch. A target with no
//! registered purge (its owner is not linked) is not pruned; two purges for
//! one table refuse the run. A [`RetentionSweep`] runs once before the passes,
//! for an owner's own orphan settlement. Every registration names its owning
//! extension; the job skips registrations whose owner the profile disabled.
//!
//! ```ignore
//! struct ToolExecutionsPurge;
//!
//! #[async_trait::async_trait]
//! impl RetentionPurge for ToolExecutionsPurge {
//!     fn table(&self) -> &'static str {
//!         "mcp_tool_executions"
//!     }
//!
//!     async fn purge_before(
//!         &self,
//!         ctx: &JobContext,
//!         cutoff: DateTime<Utc>,
//!         batch: i64,
//!     ) -> ProviderResult<u64> {
//!         /* delete up to `batch` rows older than `cutoff` */
//!     }
//! }
//!
//! register_retention_purge!("mcp", &ToolExecutionsPurge);
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_provider_contracts::{JobContext, ProviderResult};

/// One table's age-based delete. `#[async_trait]` keeps the trait
/// `dyn`-compatible for the registry.
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
/// `#[async_trait]` keeps the trait `dyn`-compatible for the registry.
#[async_trait]
pub trait RetentionSweep: Send + Sync {
    fn name(&self) -> &'static str;

    async fn sweep(&self, ctx: &JobContext, enforce: bool) -> ProviderResult<()>;
}

/// A [`RetentionPurge`] with the id of the extension that owns its table.
#[derive(Clone, Copy)]
pub struct RetentionPurgeRegistration {
    pub owner: &'static str,
    pub purge: &'static dyn RetentionPurge,
}

/// A [`RetentionSweep`] with the id of the extension that owns it.
#[derive(Clone, Copy)]
pub struct RetentionSweepRegistration {
    pub owner: &'static str,
    pub sweep: &'static dyn RetentionSweep,
}

impl std::fmt::Debug for RetentionPurgeRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetentionPurgeRegistration")
            .field("owner", &self.owner)
            .field("table", &self.purge.table())
            .finish()
    }
}

impl std::fmt::Debug for RetentionSweepRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetentionSweepRegistration")
            .field("owner", &self.owner)
            .field("sweep", &self.sweep.name())
            .finish()
    }
}

inventory::collect!(RetentionPurgeRegistration);
inventory::collect!(RetentionSweepRegistration);

pub fn registered_retention_purges() -> impl Iterator<Item = &'static RetentionPurgeRegistration> {
    inventory::iter::<RetentionPurgeRegistration>.into_iter()
}

pub fn registered_retention_sweeps() -> impl Iterator<Item = &'static RetentionSweepRegistration> {
    inventory::iter::<RetentionSweepRegistration>.into_iter()
}

#[macro_export]
macro_rules! register_retention_purge {
    ($owner:literal, $purge:expr) => {
        $crate::__inventory::submit! {
            $crate::retention_purge::RetentionPurgeRegistration {
                owner: $owner,
                purge: $purge as &'static dyn $crate::retention_purge::RetentionPurge,
            }
        }
    };
}

#[macro_export]
macro_rules! register_retention_sweep {
    ($owner:literal, $sweep:expr) => {
        $crate::__inventory::submit! {
            $crate::retention_purge::RetentionSweepRegistration {
                owner: $owner,
                sweep: $sweep as &'static dyn $crate::retention_purge::RetentionSweep,
            }
        }
    };
}
