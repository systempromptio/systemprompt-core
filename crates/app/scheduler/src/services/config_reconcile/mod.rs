//! Drift reporting for declared configuration planes.
//!
//! [`ConfigReconcileService`] walks every registered
//! [`systemprompt_provider_contracts::ConfigPlane`] in dependency order (see
//! [`ConfigPlaneSet`]), diffs each declaration against the database and
//! reports the drift. It never writes a plane's rows itself: a `db`-authority
//! declaration reaches the database only through an explicit apply
//! ([`crate::services::ConfigApplyService`], `config apply` in the deploy
//! pipeline or `/admin/sync`) or the operator's `admin config planes apply`,
//! and a `file`-authority plane is overwritten from its file by
//! [`crate::services::ConfigApplyService::apply_file_managed`] just before
//! each pass. A plane with no declaration file is reported as `undeclared`.
//!
//! The pass runs at boot (a plane whose declaration does not load fails the
//! boot) and as the `config_reconcile` job. Each pass rewrites
//! `config_plane_state` and the `systemprompt_config_drift_rows` /
//! `systemprompt_config_plane_info` gauges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod metrics;
mod report;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_provider_contracts::{ConfigPlane, Dependencies};
use systemprompt_runtime::{AppContext, ConfigPlaneSet};

use crate::error::{SchedulerError, SchedulerResult};
use crate::repository::{ConfigReconcileRepository, PlaneStateRow, PlaneStateUpdate};

pub use metrics::{CONFIG_DRIFT_ROWS, CONFIG_PLANE_INFO};
pub use report::{
    AppliedPlane, PlaneDiff, PlaneOutcome, PlaneReport, ReconcileReport, ReconcileTrigger,
};

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
            .with(ctx.services_state().view())
            .with(ctx.secrets().get())
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
        let previous: HashMap<String, PlaneStateRow> = match self.repository.list_states().await {
            Ok(rows) => rows.into_iter().map(|r| (r.plane.clone(), r)).collect(),
            Err(error) => {
                tracing::warn!(error = %error, "config_plane_state could not be read");
                HashMap::new()
            },
        };
        let mut report = ReconcileReport::default();
        for plane in self.planes.planes() {
            let plane_report = self.reconcile_plane(plane.as_ref(), trigger).await;
            metrics::record(&plane_report, previous.get(&plane_report.plane));
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
            outcome: PlaneOutcome::Undeclared,
            error: None,
        };
        if let Err(error) = self.reconcile_into(plane, &mut report).await {
            tracing::error!(
                plane = plane.name(),
                trigger = ?trigger,
                error = %error,
                "config plane reconcile failed"
            );
            report.outcome = PlaneOutcome::Failed;
            report.error = Some(error.to_string());
        }
        report
    }

    async fn reconcile_into(
        &self,
        plane: &dyn ConfigPlane,
        report: &mut PlaneReport,
    ) -> SchedulerResult<()> {
        let Some(declared) = plane.load_declared(self.planes.services_root())? else {
            return Ok(());
        };
        report.declared_hash = Some(declared.hash.clone());
        let mut tx = self.repository.begin().await?;
        let current = self.read_current(plane, &mut tx).await?;
        tx.rollback().await?;
        let drift = plane.diff(&declared, &current);
        report.drift_rows = drift.len();
        report.outcome = if drift.is_empty() {
            PlaneOutcome::InSync
        } else {
            tracing::warn!(
                plane = plane.name(),
                drift_rows = drift.len(),
                "config plane drift reported; nothing written (run `config apply`)"
            );
            PlaneOutcome::DriftReported
        };
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
        };
        if let Err(error) = self.repository.upsert_state(&update).await {
            tracing::warn!(plane = %report.plane, error = %error, "config_plane_state not recorded");
        }
    }
}
