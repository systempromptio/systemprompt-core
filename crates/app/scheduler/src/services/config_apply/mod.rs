//! The explicit apply of declared configuration to the database
//! (credential.md §3.1 rule 3): `config apply` in the deploy pipeline and the
//! apply on `/admin/sync`. Boot never runs it.
//!
//! [`ConfigApplyService::apply`] takes the same advisory lock the reconcile
//! pass takes, so one applier runs at a time across every replica; a second
//! applier waits, then finds the first one's base and has nothing to do. Each
//! plane is merged entity by entity against `config_plane_base` (see
//! [`merge`]) on one transaction: the rows the file changed are written under
//! the change context (actor, reason and the entity's declared origin), each
//! written entity is audited in `config_change_audit` with mode `merge`,
//! entities both sides changed are recorded in `config_conflicts` and left
//! alone, the base moves for every entity the apply settled, and the plane's
//! `last_apply_*` columns are stamped. A dry run does all of that and rolls it
//! back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod export;
pub mod merge;
mod plane;

use std::time::Duration;

use serde::Serialize;
use systemprompt_provider_contracts::{ApplyOutcome, ChangeActor};

use crate::error::{SchedulerError, SchedulerResult};
use crate::services::ConfigReconcileService;

pub use export::PlanePatch;
pub use merge::{EntityMerge, Verdict, merge_plane, verdict};

pub const APPLY_MODE: &str = "merge";

/// What an apply runs over and on whose behalf.
#[derive(Debug, Clone)]
pub struct ApplyOptions<'a> {
    pub planes: &'a [String],
    pub actor: ChangeActor,
    pub reason: &'a str,
    pub dry_run: bool,
    pub wait: Duration,
}

/// One plane's result: the entities the apply wrote, settled without writing,
/// kept as drift and left in conflict.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlaneMerge {
    pub plane: String,
    pub declared_hash: Option<String>,
    pub adopted: bool,
    pub applied: Vec<String>,
    pub converged: Vec<String>,
    pub drift: Vec<String>,
    pub conflicts: Vec<String>,
    pub unchanged: usize,
    pub outcome: ApplyOutcome,
    pub undeclared: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MergeReport {
    pub dry_run: bool,
    pub planes: Vec<PlaneMerge>,
}

impl MergeReport {
    #[must_use]
    pub fn applied(&self) -> usize {
        self.planes.iter().map(|p| p.applied.len()).sum()
    }

    #[must_use]
    pub fn conflicts(&self) -> usize {
        self.planes.iter().map(|p| p.conflicts.len()).sum()
    }
}

#[derive(Debug, Clone)]
pub struct ConfigApplyService {
    reconcile: ConfigReconcileService,
}

impl ConfigApplyService {
    #[must_use]
    pub const fn new(reconcile: ConfigReconcileService) -> Self {
        Self { reconcile }
    }

    #[must_use]
    pub const fn reconcile(&self) -> &ConfigReconcileService {
        &self.reconcile
    }

    pub async fn apply(&self, options: &ApplyOptions<'_>) -> SchedulerResult<MergeReport> {
        let guard = self
            .reconcile
            .repository()
            .acquire_reconcile_lock(options.wait)
            .await?
            .ok_or(SchedulerError::ReconcileLockTimeout {
                waited_secs: options.wait.as_secs(),
            })?;
        let report = self.apply_locked(options).await;
        guard.release().await;
        report
    }

    async fn apply_locked(&self, options: &ApplyOptions<'_>) -> SchedulerResult<MergeReport> {
        let set = self.reconcile.planes();
        for name in options.planes {
            set.find(name)?;
        }
        let mut report = MergeReport {
            dry_run: options.dry_run,
            planes: Vec::new(),
        };
        for plane in set.planes() {
            if !options.planes.is_empty() && !options.planes.iter().any(|n| n == plane.name()) {
                continue;
            }
            report
                .planes
                .push(self.merge_one(plane.as_ref(), options).await?);
        }
        Ok(report)
    }
}
