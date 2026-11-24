//! Configured-owner inventory refresh: reconciles the configured and managed
//! inventory, publishes the latest configured revision of every available
//! skill, and recomputes installation coverage.
//!
//! The `managed_resources` member contributes the job. It ticks every minute
//! on every node but only reconciles when the configured inventory could have
//! changed. It fingerprints what a refresh observes
//! (`configured_inventory_fingerprint`: the loaded services configuration
//! plus path, mtime and size of every catalog file) and skips the reconcile
//! and publish while the fingerprint matches the last successful pass the job
//! value remembers. A pass older than an hour runs regardless, as a safety
//! net for managed state changed outside the catalog. The first tick after
//! boot always runs.
//!
//! Installation coverage depends on receipts, grants, publication selections
//! and enrolled devices rather than the catalog: the marketplace schema's
//! statement triggers queue the owners those changes touched on the
//! `managed_installation_coverage` projection, and every tick drains that
//! queue through `refresh_managed_installation_coverage` (rebuilding only
//! queued owners, writing only the bodies that changed).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use systemprompt_database::{DbPool, projection};
use systemprompt_traits::{
    Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult, StateRequirement,
};

use super::{InventoryRefresher, LatestPublicationStatus};

const FORCE_AFTER: Duration = Duration::from_hours(1);
const COVERAGE_PROJECTION: &str = "managed_installation_coverage";
const COVERAGE_PROJECTOR: &str = "refresh_managed_installation_coverage";
const COVERAGE_BATCH: i32 = 20;
const COVERAGE_BUDGET: Duration = Duration::from_secs(20);

#[derive(Debug, Default)]
struct RefreshMemo {
    last: Mutex<Option<([u8; 32], Instant)>>,
}

impl RefreshMemo {
    fn unchanged(&self, fingerprint: &[u8; 32]) -> bool {
        let guard = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        guard
            .as_ref()
            .is_some_and(|(seen, at)| seen == fingerprint && at.elapsed() < FORCE_AFTER)
    }

    fn remember(&self, fingerprint: [u8; 32]) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) =
            Some((fingerprint, Instant::now()));
    }
}

/// The `managed_inventory_refresh` job and the last successful pass it
/// remembers.
#[derive(Debug, Clone, Default)]
pub struct InventoryRefreshJob {
    memo: Arc<RefreshMemo>,
}

fn internal(error: impl std::error::Error + Send + Sync + 'static) -> ProviderError {
    ProviderError::Internal(Box::new(error))
}

#[async_trait]
impl Job for InventoryRefreshJob {
    fn name(&self) -> &'static str {
        "managed_inventory_refresh"
    }
    fn description(&self) -> &'static str {
        "Reconciles configured and managed inventory when the catalog changed, publishes the latest configured skills and recomputes installation coverage"
    }
    fn schedule(&self) -> &'static str {
        "0 * * * * *"
    }
    fn scope(&self) -> JobScope {
        JobScope::Node
    }
    fn requires(&self) -> Vec<StateRequirement> {
        vec![
            StateRequirement::of::<Arc<InventoryRefresher>>(),
            StateRequirement::of::<DbPool>(),
        ]
    }
    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let refresher = ctx.get::<Arc<InventoryRefresher>>()?;
        let fingerprint = refresher.fingerprint();
        let published = match fingerprint {
            Ok(fingerprint) if self.memo.unchanged(&fingerprint) => 0,
            fingerprint => {
                let outcomes = refresher.publish_as_owner().await.map_err(internal)?;
                match fingerprint {
                    Ok(fingerprint) => self.memo.remember(fingerprint),
                    Err(error) => {
                        tracing::warn!(%error, "Inventory published without a fingerprint; the next run refreshes again");
                    },
                }
                let count = outcomes
                    .iter()
                    .filter(|outcome| outcome.status == LatestPublicationStatus::Published)
                    .count();
                u64::try_from(count).unwrap_or(u64::MAX)
            },
        };
        let db_pool = Arc::clone(ctx.get::<DbPool>()?);
        let coverage = projection::drain_with_sql(
            &db_pool.write_pool(),
            COVERAGE_PROJECTION,
            COVERAGE_BATCH,
            COVERAGE_BUDGET,
            COVERAGE_PROJECTOR,
        )
        .await
        .map_err(internal)?;
        Ok(JobResult::success().with_stats(published + coverage.keys, coverage.requeued))
    }
}
