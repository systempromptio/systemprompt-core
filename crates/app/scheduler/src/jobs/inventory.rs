//! Configured-owner inventory refresh: reconciles the configured and managed
//! inventory, publishes the latest configured revision of every available
//! skill, and recomputes installation coverage.
//!
//! The job ticks every minute on every node but only reconciles when the
//! configured inventory could have changed. It fingerprints what a refresh
//! observes ([`configured_inventory_fingerprint`]: the loaded services
//! configuration plus path, mtime and size of every catalog file) and skips
//! the reconcile and publish while the fingerprint matches the last
//! successful pass in this process. A pass older than [`FORCE_AFTER`] runs
//! regardless, as a safety net for managed state changed outside the
//! catalog. The first tick after boot always runs.
//!
//! Installation coverage depends on receipts, grants and publications rather
//! than the catalog, so it is recomputed every tick; it writes only when a
//! resource's coverage changed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::SchedulerError;
use async_trait::async_trait;
use systemprompt_marketplace::inventory::configured_inventory_fingerprint;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::managed::inventory::LatestPublicationStatus;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderResult};

const FORCE_AFTER: Duration = Duration::from_hours(1);

type LastPass = Option<([u8; 32], Instant)>;

fn last_pass() -> &'static Mutex<LastPass> {
    static LAST: OnceLock<Mutex<LastPass>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
}

#[derive(Debug, Clone, Copy)]
pub struct InventoryRefreshJob;

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
    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let app = ctx
            .app_context::<Arc<AppContext>>()
            .ok_or_else(|| SchedulerError::missing_context("AppContext"))?;
        let owner = app.system_admin().id();
        let published = match catalog_fingerprint(app) {
            Some(fingerprint) if unchanged(&fingerprint) => 0,
            fingerprint => {
                // Why: the owner is the system admin because that is who owns
                // the configured inventory.
                let outcomes = systemprompt_runtime::managed::inventory::publish_latest(
                    app,
                    owner,
                    &ctx.actor().user_id,
                )
                .await
                .map_err(|error| SchedulerError::config_error(error.to_string()))?;
                if let Some(fingerprint) = fingerprint {
                    remember(fingerprint);
                }
                let count = outcomes
                    .iter()
                    .filter(|outcome| outcome.status == LatestPublicationStatus::Published)
                    .count();
                u64::try_from(count).unwrap_or(u64::MAX)
            },
        };
        let coverage_changed = app
            .managed_repository()
            .refresh_installation_coverage(owner)
            .await
            .map_err(|error| SchedulerError::config_error(error.to_string()))?;
        Ok(JobResult::success().with_stats(published + coverage_changed, 0))
    }
}

// Why: a configuration that does not load has no fingerprint; the refresh
// then runs and records the failure against the inventory, as before.
fn catalog_fingerprint(app: &AppContext) -> Option<[u8; 32]> {
    let services = systemprompt_loader::ConfigLoader::load().ok()?;
    configured_inventory_fingerprint(app.app_paths().system().services(), &services)
        .inspect_err(|error| tracing::debug!(%error, "Inventory fingerprint unavailable"))
        .ok()
}

fn unchanged(fingerprint: &[u8; 32]) -> bool {
    let guard = last_pass()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .as_ref()
        .is_some_and(|(seen, at)| seen == fingerprint && at.elapsed() < FORCE_AFTER)
}

fn remember(fingerprint: [u8; 32]) {
    *last_pass()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((fingerprint, Instant::now()));
}

systemprompt_provider_contracts::submit_job!(&InventoryRefreshJob);
