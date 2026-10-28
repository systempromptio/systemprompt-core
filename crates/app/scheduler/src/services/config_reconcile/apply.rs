//! Operator-facing diff, export and apply, and the audited apply every write
//! shares.
//!
//! Every plane read and write runs on one write-pool transaction. An apply
//! re-reads the plane on that transaction, writes one `config_change_audit`
//! row per admitted entity whose value actually changed into it, and only the
//! caller's commit makes change and audit visible; an apply that must
//! converge and did not is an error, and the dropped transaction rolls the
//! plane's writes back with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::{
    ApplyMode, ApplyOutcome, ApplyRequest, ChangeRequest, ConfigPlane, ConfigPlaneError,
    DeclaredState, DriftRow, EntityScope, PlaneContext, PlaneSnapshot, PlaneTx,
};

use super::{AppliedPlane, ConfigReconcileService, PlaneDiff};
use crate::error::{SchedulerError, SchedulerResult};
use crate::repository::{ConfigChangeRecord, PlaneTransaction};

#[derive(Clone, Copy)]
pub(super) struct Observed<'a> {
    pub(super) declared: &'a DeclaredState,
    pub(super) current: &'a PlaneSnapshot,
    pub(super) drift: &'a [DriftRow],
}

#[derive(Clone, Copy)]
enum Finish {
    Commit,
    Rollback,
}

#[derive(Clone, Copy)]
pub(super) struct Planned<'a> {
    pub(super) mode: ApplyMode,
    pub(super) change: &'a ChangeRequest<'a>,
    pub(super) scope: EntityScope<'a>,
}

impl ConfigReconcileService {
    pub(crate) fn plane_context(&self) -> PlaneContext<'_> {
        PlaneContext::new(self.planes.services_root(), &self.dependencies)
    }

    pub(super) async fn read_current(
        &self,
        plane: &dyn ConfigPlane,
        tx: &mut PlaneTransaction,
    ) -> Result<PlaneSnapshot, ConfigPlaneError> {
        let ctx = self.plane_context();
        let mut plane_tx = PlaneTx::new(tx);
        plane.read_current(&ctx, &mut plane_tx).await
    }

    pub async fn diff(&self, name: &str) -> SchedulerResult<PlaneDiff> {
        let plane = self.planes.find(name)?;
        let declared = plane.load_declared(self.planes.services_root())?;
        let rows = match &declared {
            Some(declared) => {
                let mut tx = self.repository.begin().await?;
                let current = self.read_current(plane.as_ref(), &mut tx).await?;
                plane.diff(declared, &current)
            },
            None => Vec::new(),
        };
        Ok(PlaneDiff {
            plane: plane.name().to_owned(),
            authority: self.planes.authority_of(plane.as_ref()),
            effect: plane.effect(),
            declared_path: plane.declared_path().to_owned(),
            declared_hash: declared.map(|d| d.hash),
            rows,
        })
    }

    pub async fn export(&self, name: &str) -> SchedulerResult<Option<String>> {
        let plane = self.planes.find(name)?;
        let mut tx = self.repository.begin().await?;
        let ctx = self.plane_context();
        let mut plane_tx = PlaneTx::new(&mut tx);
        Ok(plane.export(&ctx, &mut plane_tx).await?)
    }

    pub async fn apply(
        &self,
        name: &str,
        mode: ApplyMode,
        change: &ChangeRequest<'_>,
        scope: EntityScope<'_>,
    ) -> SchedulerResult<AppliedPlane> {
        let planned = Planned {
            mode,
            change,
            scope,
        };
        self.apply_locked(name, planned, Finish::Commit).await
    }

    pub async fn preview_apply(
        &self,
        name: &str,
        mode: ApplyMode,
        change: &ChangeRequest<'_>,
        scope: EntityScope<'_>,
    ) -> SchedulerResult<AppliedPlane> {
        let planned = Planned {
            mode,
            change,
            scope,
        };
        self.apply_locked(name, planned, Finish::Rollback).await
    }

    async fn apply_locked(
        &self,
        name: &str,
        planned: Planned<'_>,
        finish: Finish,
    ) -> SchedulerResult<AppliedPlane> {
        let guard = self
            .repository
            .acquire_reconcile_lock(super::RECONCILE_LOCK_WAIT)
            .await?
            .ok_or(SchedulerError::ReconcileLockTimeout {
                waited_secs: super::RECONCILE_LOCK_WAIT.as_secs(),
            })?;
        let applied = self.apply_unlocked(name, planned, finish).await;
        guard.release().await;
        applied
    }

    async fn apply_unlocked(
        &self,
        name: &str,
        planned: Planned<'_>,
        finish: Finish,
    ) -> SchedulerResult<AppliedPlane> {
        let plane = self.planes.find(name)?;
        let declared = plane
            .load_declared(self.planes.services_root())?
            .ok_or_else(|| ConfigPlaneError::Invalid {
                plane: name.to_owned(),
                path: plane.declared_path().to_owned(),
                message: "the declaration file does not exist".to_owned(),
            })?;
        let mut tx = self.repository.begin().await?;
        let current = self.read_current(plane.as_ref(), &mut tx).await?;
        let drift = plane.diff(&declared, &current);
        let observed = Observed {
            declared: &declared,
            current: &current,
            drift: &drift,
        };
        let applied = self
            .apply_recorded(plane.as_ref(), &mut tx, observed, planned)
            .await?;
        match finish {
            Finish::Commit => tx.commit().await?,
            Finish::Rollback => tx.rollback().await?,
        }
        Ok(applied)
    }

    pub(super) async fn apply_recorded(
        &self,
        plane: &dyn ConfigPlane,
        tx: &mut PlaneTransaction,
        observed: Observed<'_>,
        planned: Planned<'_>,
    ) -> SchedulerResult<AppliedPlane> {
        let Observed {
            declared,
            current,
            drift,
        } = observed;
        let Planned {
            mode,
            change,
            scope,
        } = planned;
        if !scope.is_all() && !plane.supports_scoped_apply() {
            return Err(ConfigPlaneError::Unsupported {
                plane: plane.name().to_owned(),
                operation: "a scoped apply",
            }
            .into());
        }
        if mode == ApplyMode::SeedIfEmpty && !current.is_empty() {
            return Ok(AppliedPlane {
                plane: plane.name().to_owned(),
                declared_hash: declared.hash.clone(),
                outcome: ApplyOutcome::default(),
                changes: 0,
                remaining_drift: drift.iter().filter(|r| scope.admits(&r.entity)).count(),
            });
        }
        let request = ApplyRequest {
            declared,
            mode,
            change,
            scope,
        };
        let ctx = self.plane_context();
        let (outcome, after) = {
            let mut plane_tx = PlaneTx::new(&mut *tx);
            let outcome = plane.apply(&ctx, &mut plane_tx, &request).await?;
            (outcome, plane.read_current(&ctx, &mut plane_tx).await?)
        };
        let remaining_drift = plane
            .diff(declared, &after)
            .iter()
            .filter(|r| scope.admits(&r.entity))
            .count();
        if mode.must_converge() && remaining_drift > 0 {
            return Err(ConfigPlaneError::NotConverged {
                plane: plane.name().to_owned(),
                remaining: remaining_drift,
            }
            .into());
        }
        let records = change_records(plane.name(), drift, &after, &request);
        self.repository.record_changes(tx, &records).await?;
        Ok(AppliedPlane {
            plane: plane.name().to_owned(),
            declared_hash: declared.hash.clone(),
            outcome,
            changes: records.len(),
            remaining_drift,
        })
    }
}

fn change_records<'a>(
    plane: &'a str,
    drift: &'a [DriftRow],
    after: &'a PlaneSnapshot,
    request: &'a ApplyRequest<'a>,
) -> Vec<ConfigChangeRecord<'a>> {
    drift
        .iter()
        .filter(|row| request.scope.admits(&row.entity))
        .filter_map(|row| {
            let after_value = after.get(&row.entity).map(String::as_str);
            (after_value != row.current.as_deref()).then(|| ConfigChangeRecord {
                plane,
                entity: &row.entity,
                change_kind: row.kind.as_str(),
                before_value: row.current.as_deref(),
                after_value,
                mode: request.mode.as_str(),
                actor: request.change.actor.label(),
                reason: request.change.reason,
                declared_hash: &request.declared.hash,
            })
        })
        .collect()
}
