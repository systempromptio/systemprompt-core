//! The explicit apply of declared configuration to the database.
//!
//! This is `config apply` in the deploy pipeline and the apply on
//! `/admin/sync`, plus the file-wins pass the scheduler node runs
//! for `file`-authority planes at boot and on every `config_reconcile` tick
//! ([`ConfigApplyService::apply_file_managed`]).
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
//! `last_apply_*` columns are stamped. A plane under effective `file`
//! authority merges with `file_wins` instead: the database is overwritten
//! with the declaration (audited with mode `overwrite`), and every open
//! conflict on an entity it writes is resolved as `file`, so the apply
//! converges without an operator. Every plane the apply ran, declared or
//! not, is recorded in `config_plane_applied`, so its runtime reader serves a
//! declared absence instead of failing closed as unapplied. A dry run does all
//! of that and rolls it back.
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
pub use merge::{EntityMerge, Verdict, file_wins_verdict, merge_plane, verdict};

pub const APPLY_MODE: &str = "merge";
pub const FILE_WINS_MODE: &str = "overwrite";

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
    pub file_wins: bool,
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

    pub async fn apply_file_managed(
        &self,
        actor: ChangeActor,
        reason: &str,
        wait: Duration,
    ) -> SchedulerResult<MergeReport> {
        let set = self.reconcile.planes();
        let planes: Vec<String> = set
            .planes()
            .iter()
            .filter(|p| set.authority_of(p.as_ref()).is_file_managed())
            .map(|p| p.name().to_owned())
            .collect();
        if planes.is_empty() {
            return Ok(MergeReport::default());
        }
        self.apply(&ApplyOptions {
            planes: &planes,
            actor,
            reason,
            dry_run: false,
            wait,
        })
        .await
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
