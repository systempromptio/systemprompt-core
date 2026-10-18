//! Desired-state reconciliation of declared configuration planes.
//!
//! [`ConfigReconcileService`] walks every registered
//! [`systemprompt_provider_contracts::ConfigPlane`] in registry order. A plane
//! under `file` authority whose declaration exists is overwritten back to the
//! declaration whenever the database drifts; a `db` plane is seeded from its
//! file only while its projection is empty and otherwise only reported; a
//! plane with no declaration file is reported as `undeclared` and never
//! written. Every apply re-reads the database and records one
//! `config_change_audit` row per entity whose value actually changed, with
//! the before and after values; an overwrite that leaves drift behind is a
//! failure, not a correction.
//!
//! The pass runs at boot (a failed plane fails the boot) and as the
//! `config_reconcile` job. Each pass rewrites `config_plane_state` and the
//! `systemprompt_config_drift_rows` / `systemprompt_config_plane_info` gauges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod metrics;
mod report;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_provider_contracts::{
    ApplyMode, ChangeActor, ChangeRequest, ConfigPlane, ConfigPlaneError, DeclaredState,
    Dependencies, PlaneContext, PlaneSnapshot,
};
use systemprompt_runtime::{AppContext, ConfigPlaneSet, EffectiveAuthority};

use crate::error::{SchedulerError, SchedulerResult};
use crate::repository::{ConfigReconcileRepository, PlaneStateUpdate};

pub use metrics::{CONFIG_DRIFT_ROWS, CONFIG_PLANE_INFO};
pub use report::{
    AppliedPlane, PlaneDiff, PlaneOutcome, PlaneReport, ReconcileReport, ReconcileTrigger,
};

const RECONCILE_REASON: &str = "declared file is authoritative; database drift overwritten";
const SEED_REASON: &str = "empty projection seeded from its declaration";
pub const RECONCILE_LOCK_WAIT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct ConfigReconcileService {
    planes: ConfigPlaneSet,
    repository: ConfigReconcileRepository,
    dependencies: Dependencies,
}

impl ConfigReconcileService {
    #[must_use]
    pub const fn new(
        planes: ConfigPlaneSet,
        repository: ConfigReconcileRepository,
        dependencies: Dependencies,
    ) -> Self {
        Self {
            planes,
            repository,
            dependencies,
        }
    }

    pub fn from_app_context(
        ctx: &Arc<AppContext>,
        repository: ConfigReconcileRepository,
    ) -> SchedulerResult<Self> {
        let dependencies = Dependencies::new()
            .with(Arc::clone(ctx.db_pool()))
            .with(Arc::clone(ctx));
        Ok(Self::new(ctx.config_planes()?, repository, dependencies))
    }

    #[must_use]
    pub const fn planes(&self) -> &ConfigPlaneSet {
        &self.planes
    }

    #[must_use]
    pub const fn repository(&self) -> &ConfigReconcileRepository {
        &self.repository
    }

    pub async fn reconcile_exclusive(
        &self,
        trigger: ReconcileTrigger,
        wait: Duration,
    ) -> SchedulerResult<ReconcileReport> {
        let guard = self.repository.acquire_reconcile_lock(wait).await?.ok_or(
            SchedulerError::ReconcileLockTimeout {
                waited_secs: wait.as_secs(),
            },
        )?;
        let report = self.reconcile_all(trigger).await;
        guard.release().await;
        Ok(report)
    }

    pub async fn reconcile_all(&self, trigger: ReconcileTrigger) -> ReconcileReport {
        for key in self.planes.unregistered_authority_keys() {
            tracing::warn!(plane = %key, "config_planes.authority names a plane no extension registers");
        }
        let previous: HashMap<String, Option<String>> = match self.repository.list_states().await {
            Ok(rows) => rows
                .into_iter()
                .map(|r| (r.plane, r.declared_hash))
                .collect(),
            Err(error) => {
                tracing::warn!(error = %error, "config_plane_state could not be read");
                HashMap::new()
            },
        };
        let mut report = ReconcileReport::default();
        for plane in self.planes.planes() {
            let plane_report = self.reconcile_plane(plane.as_ref(), trigger).await;
            metrics::record(
                &plane_report,
                previous.get(&plane_report.plane).and_then(Option::as_deref),
            );
            self.persist(&plane_report).await;
            report.planes.push(plane_report);
        }
        report
    }

    async fn reconcile_plane(
        &self,
        plane: &dyn ConfigPlane,
        trigger: ReconcileTrigger,
    ) -> PlaneReport {
        let authority = self.planes.authority_of(plane);
        let mut report = PlaneReport {
            plane: plane.name().to_owned(),
            authority,
            declared_hash: None,
            drift_rows: 0,
            changes: 0,
            outcome: PlaneOutcome::Undeclared,
            error: None,
        };
        if let Err(error) = self.reconcile_into(plane, trigger, &mut report).await {
            tracing::error!(plane = plane.name(), error = %error, "config plane reconcile failed");
            report.outcome = PlaneOutcome::Failed;
            report.error = Some(error.to_string());
        }
        report
    }

    async fn reconcile_into(
        &self,
        plane: &dyn ConfigPlane,
        trigger: ReconcileTrigger,
        report: &mut PlaneReport,
    ) -> Result<(), ConfigPlaneError> {
        let Some(declared) = plane.load_declared(self.planes.services_root())? else {
            return Ok(());
        };
        report.declared_hash = Some(declared.hash.clone());
        let ctx = PlaneContext::new(self.planes.services_root(), &self.dependencies);
        let current = plane.read_current(&ctx).await?;
        let drift = plane.diff(&declared, &current);
        report.drift_rows = drift.len();
        refuse_unattended_wipe(plane, report.authority, &declared, &current)?;
        let actor = match trigger {
            ReconcileTrigger::Boot => ChangeActor::Boot,
            ReconcileTrigger::Scheduled => ChangeActor::Reconciler,
        };
        let (outcome, reason) = match report.authority {
            EffectiveAuthority::File if !drift.is_empty() => {
                (PlaneOutcome::Corrected, RECONCILE_REASON)
            },
            EffectiveAuthority::Db | EffectiveAuthority::Undeclared
                if current.is_empty() && !declared.entries.is_empty() =>
            {
                (PlaneOutcome::Seeded, SEED_REASON)
            },
            _ if drift.is_empty() => {
                report.outcome = PlaneOutcome::InSync;
                return Ok(());
            },
            _ => {
                tracing::warn!(
                    plane = plane.name(),
                    drift_rows = drift.len(),
                    "config plane drift reported; database authority, nothing written"
                );
                report.outcome = PlaneOutcome::DriftReported;
                return Ok(());
            },
        };
        let change = ChangeRequest { actor, reason };
        let applied = self
            .apply_recorded(
                plane,
                &ctx,
                apply::Observed {
                    declared: &declared,
                    drift: &drift,
                    mode: ApplyMode::Overwrite,
                },
                &change,
            )
            .await?;
        if applied.remaining_drift > 0 {
            return Err(ConfigPlaneError::NotConverged {
                plane: plane.name().to_owned(),
                remaining: applied.remaining_drift,
            });
        }
        tracing::info!(
            plane = plane.name(),
            outcome = outcome.as_str(),
            changes = applied.changes,
            hash = %declared.hash,
            "config plane reconciled to its declaration"
        );
        report.outcome = outcome;
        report.changes = applied.changes;
        report.drift_rows = 0;
        Ok(())
    }

    async fn persist(&self, report: &PlaneReport) {
        let update = PlaneStateUpdate {
            plane: &report.plane,
            authority: report.authority.as_str(),
            declared_hash: report.declared_hash.as_deref(),
            drift_rows: i32::try_from(report.drift_rows).unwrap_or(i32::MAX),
            outcome: report.outcome.as_str(),
            last_error: report.error.as_deref(),
            corrected: report.changes > 0,
        };
        if let Err(error) = self.repository.upsert_state(&update).await {
            tracing::warn!(plane = %report.plane, error = %error, "config_plane_state not recorded");
        }
    }
}

fn refuse_unattended_wipe(
    plane: &dyn ConfigPlane,
    authority: EffectiveAuthority,
    declared: &DeclaredState,
    current: &PlaneSnapshot,
) -> Result<(), ConfigPlaneError> {
    if !authority.is_file_managed() || !declared.entries.is_empty() || current.is_empty() {
        return Ok(());
    }
    Err(ConfigPlaneError::Invalid {
        plane: plane.name().to_owned(),
        path: plane.declared_path().to_owned(),
        message: format!(
            "the declaration is empty and would delete all {} database rows; refusing an \
             unattended wipe (use `admin config planes apply` to empty the plane)",
            current.len()
        ),
    })
}
