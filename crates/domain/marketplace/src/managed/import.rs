//! Capture import creates durable candidates without activating them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    CapturedSkills, ManagedRepository, NewResource, NewRevision, ResourceKind, Result,
    SnapshotProvenance,
};
use serde::Serialize;
use std::collections::BTreeMap;
use systemprompt_identifiers::{ManagedSourceId, ResourceRevisionId, SourceSnapshotId, UserId};

#[derive(Debug, Clone, Serialize)]
pub struct ImportedSkills {
    pub source_id: ManagedSourceId,
    pub snapshot_id: SourceSnapshotId,
    pub revisions: BTreeMap<String, ResourceRevisionId>,
}

impl ManagedRepository {
    pub async fn import_skills(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
        captured: &CapturedSkills,
        commit: Option<String>,
    ) -> Result<ImportedSkills> {
        let spec = self.get_source(owner, source).await?;
        let snapshot = self
            .capture_snapshot(
                owner,
                source,
                &SnapshotProvenance {
                    source_kind: spec.kind().to_owned(),
                    commit,
                    tree_digest: captured.tree_digest.clone(),
                    importer_version: format!("services-skills-v1/{}", env!("CARGO_PKG_VERSION")),
                },
            )
            .await?;
        let mut revisions = BTreeMap::new();
        for (key, files) in &captured.skills {
            let resource = self
                .bind_resource(
                    owner,
                    &NewResource {
                        source_id: source.clone(),
                        upstream_key: format!("skills/{key}"),
                        kind: ResourceKind::Skill,
                        resource_key: key.clone(),
                    },
                )
                .await?;
            let revision = self
                .create_revision(
                    owner,
                    &NewRevision {
                        resource_id: resource,
                        snapshot_id: snapshot.clone(),
                        parent_id: None,
                        files: files.clone(),
                        dependencies: BTreeMap::new(),
                        rationale: "Import immutable authoring snapshot; no publication requested"
                            .to_owned(),
                    },
                )
                .await?;
            revisions.insert(key.clone(), revision);
        }
        Ok(ImportedSkills {
            source_id: source.clone(),
            snapshot_id: snapshot,
            revisions,
        })
    }
}
