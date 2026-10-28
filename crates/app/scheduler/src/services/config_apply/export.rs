//! `cloud sync export`: the database changes no apply has taken back into
//! code, rendered as declaration files for a developer to commit.
//!
//! For each plane the export runs the same merge the apply runs, read-only,
//! and takes the database side of every drifted or conflicting entity on top
//! of the current declaration. The result is the plane's whole declaration
//! with the console edits folded in; once it is committed and deployed, the
//! next apply sees file and database agree, records them as the base and the
//! drift clears. Nothing is written to the database or the services tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_provider_contracts::{ConfigPlane, PlaneSnapshot, PlaneTx};

use super::ConfigApplyService;
use super::merge::{Verdict, merge_plane};
use crate::error::SchedulerResult;

/// One plane's exported declaration: where it belongs under the services
/// tree, the entities whose database value it carries, and the file text.
#[derive(Debug, Clone, Serialize)]
pub struct PlanePatch {
    pub plane: String,
    pub declared_path: String,
    pub entities: Vec<String>,
    pub text: String,
}

impl ConfigApplyService {
    pub async fn export_patches(&self, planes: &[String]) -> SchedulerResult<Vec<PlanePatch>> {
        let set = self.reconcile.planes();
        for name in planes {
            set.find(name)?;
        }
        let mut patches = Vec::new();
        for plane in set.planes() {
            if !planes.is_empty() && !planes.iter().any(|n| n == plane.name()) {
                continue;
            }
            if let Some(patch) = self.export_one(plane.as_ref()).await? {
                patches.push(patch);
            }
        }
        Ok(patches)
    }

    async fn export_one(&self, plane: &dyn ConfigPlane) -> SchedulerResult<Option<PlanePatch>> {
        let set = self.reconcile.planes();
        let declared = plane.sources(set.services_root())?.unwrap_or_default();
        let repo = self.reconcile.repository();
        let ctx = self.reconcile.plane_context();
        let mut tx = repo.begin().await?;
        let current = plane.read(&ctx, &mut PlaneTx::new(&mut tx)).await?;
        let adopt = !repo.find_plane_applied(&mut tx, plane.name()).await?;
        let base = repo.list_base(&mut tx, plane.name()).await?;
        tx.rollback().await?;

        let mut patched: PlaneSnapshot = declared
            .iter()
            .map(|(k, d)| (k.clone(), d.value.clone()))
            .collect();
        let mut entities = Vec::new();
        let file_wins = set.authority_of(plane).is_file_managed();
        for merge in merge_plane(&declared, &current, &base, adopt, file_wins) {
            if !matches!(merge.verdict, Verdict::Drift | Verdict::Conflict) {
                continue;
            }
            match current.get(&merge.entity) {
                Some(value) => patched.insert(merge.entity.clone(), value.clone()),
                None => patched.remove(&merge.entity),
            };
            entities.push(merge.entity);
        }
        if entities.is_empty() {
            return Ok(None);
        }
        Ok(Some(PlanePatch {
            plane: plane.name().to_owned(),
            declared_path: plane.declared_path().to_owned(),
            entities,
            text: plane.render_declarations(&patched)?,
        }))
    }
}
