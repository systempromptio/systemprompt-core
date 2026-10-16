//! Operator-facing diff and apply, and the audited apply every write shares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::{
    ApplyMode, ChangeRequest, ConfigPlane, ConfigPlaneError, DeclaredState, DriftRow, PlaneContext,
};

use super::{AppliedPlane, ConfigReconcileService, PlaneDiff};
use crate::error::SchedulerResult;
use crate::repository::ConfigChangeRecord;

#[derive(Clone, Copy)]
pub(super) struct Observed<'a> {
    pub(super) declared: &'a DeclaredState,
    pub(super) drift: &'a [DriftRow],
    pub(super) mode: ApplyMode,
}

impl ConfigReconcileService {
    pub async fn diff(&self, name: &str) -> SchedulerResult<PlaneDiff> {
        let plane = self.planes.find(name)?;
        let declared = plane.load_declared(self.planes.services_root())?;
        let rows = match &declared {
            Some(declared) => {
                let ctx = PlaneContext::new(self.planes.services_root(), &self.dependencies);
                let current = plane.read_current(&ctx).await?;
                plane.diff(declared, &current)
            },
            None => Vec::new(),
        };
        Ok(PlaneDiff {
            plane: plane.name().to_owned(),
            authority: self.planes.authority_of(plane.as_ref()),
            declared_path: plane.declared_path().to_owned(),
            declared_hash: declared.map(|d| d.hash),
            rows,
        })
    }

    pub async fn apply(
        &self,
        name: &str,
        mode: ApplyMode,
        change: &ChangeRequest<'_>,
    ) -> SchedulerResult<AppliedPlane> {
        let plane = self.planes.find(name)?;
        let declared = plane
            .load_declared(self.planes.services_root())?
            .ok_or_else(|| ConfigPlaneError::Invalid {
                plane: name.to_owned(),
                path: plane.declared_path().to_owned(),
                message: "the declaration file does not exist".to_owned(),
            })?;
        let ctx = PlaneContext::new(self.planes.services_root(), &self.dependencies);
        let current = plane.read_current(&ctx).await?;
        let drift = plane.diff(&declared, &current);
        let applied = self
            .apply_recorded(
                plane.as_ref(),
                &ctx,
                Observed {
                    declared: &declared,
                    drift: &drift,
                    mode,
                },
                change,
            )
            .await?;
        if mode == ApplyMode::Overwrite && applied.remaining_drift > 0 {
            return Err(ConfigPlaneError::NotConverged {
                plane: name.to_owned(),
                remaining: applied.remaining_drift,
            }
            .into());
        }
        Ok(applied)
    }

    pub(super) async fn apply_recorded(
        &self,
        plane: &dyn ConfigPlane,
        ctx: &PlaneContext<'_>,
        observed: Observed<'_>,
        change: &ChangeRequest<'_>,
    ) -> Result<AppliedPlane, ConfigPlaneError> {
        let Observed {
            declared,
            drift,
            mode,
        } = observed;
        let outcome = plane.apply(ctx, declared, mode, change).await?;
        let after = plane.read_current(ctx).await?;
        let records: Vec<ConfigChangeRecord<'_>> = drift
            .iter()
            .filter_map(|row| {
                let after_value = after.get(&row.entity).map(String::as_str);
                (after_value != row.current.as_deref()).then(|| ConfigChangeRecord {
                    plane: plane.name(),
                    entity: &row.entity,
                    change_kind: row.kind.as_str(),
                    before_value: row.current.as_deref(),
                    after_value,
                    mode: mode.as_str(),
                    actor: change.actor.label(),
                    reason: change.reason,
                    declared_hash: &declared.hash,
                })
            })
            .collect();
        self.repository
            .record_changes(&records)
            .await
            .map_err(|source| ConfigPlaneError::Backend {
                plane: plane.name().to_owned(),
                source: Box::new(source),
            })?;
        Ok(AppliedPlane {
            plane: plane.name().to_owned(),
            declared_hash: declared.hash.clone(),
            outcome,
            changes: records.len(),
            remaining_drift: plane.diff(declared, &after).len(),
        })
    }
}
