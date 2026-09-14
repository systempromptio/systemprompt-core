//! Restartable authoring import reuses its exact retained snapshot identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::ApiOperation;
use crate::managed::{
    AssetDigest, CapturedSkills, ImportedSkills, ManagedError, ManagedRepository, NewResource,
    NewRevision, ResourceKind, Result, SnapshotProvenance,
};
use std::collections::BTreeMap;
use systemprompt_identifiers::{ManagedSourceId, SourceSnapshotId, UserId};
impl ManagedRepository {
    pub async fn import_api_capture(
        &self,
        owner: &UserId,
        operation: &ApiOperation,
        source: &ManagedSourceId,
        captured: &CapturedSkills,
    ) -> Result<ImportedSkills> {
        let spec = self.get_source(owner, source).await?;
        let provenance = SnapshotProvenance {
            source_kind: spec.kind().to_owned(),
            commit: None,
            tree_digest: captured.tree_digest().clone(),
            importer_version: "api-authoring-capture-v1".to_owned(),
        };
        provenance.validate(&spec)?;
        let snapshot = SourceSnapshotId::new(format!(
            "api-{}",
            AssetDigest::of(format!("{}:{}", owner, operation.id).as_bytes()).as_str()
        ));
        let digest = AssetDigest::of(&serde_jcs::to_vec(&provenance)?);
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query!(
            "SELECT fence,state FROM managed_api_operations WHERE owner_id=$1 AND id=$2 FOR SHARE",
            owner.as_str(),
            operation.id.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        if current.fence != operation.fence || current.state != "pending" {
            return Err(ManagedError::Conflict(
                "Operation lease was superseded".to_owned(),
            ));
        }
        sqlx::query!("INSERT INTO managed_source_snapshots(id,owner_id,source_id,digest,provenance) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO NOTHING",snapshot.as_str(),owner.as_str(),source.as_str(),digest.as_str(),sqlx::types::Json(&provenance) as _).execute(&mut *tx).await?;
        let stored = sqlx::query_scalar!(
            "SELECT digest FROM managed_source_snapshots WHERE id=$1 AND owner_id=$2",
            snapshot.as_str(),
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        if stored != digest.as_str() {
            return Err(ManagedError::Conflict(
                "Retained capture differs from restart input".to_owned(),
            ));
        }
        tx.commit().await?;
        let mut revisions = BTreeMap::new();
        for (key, files) in captured.skills() {
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
            let revision=self.create_revision(owner,&NewRevision{resource_id:resource,snapshot_id:snapshot.clone(),parent_id:None,files:files.clone(),dependencies:BTreeMap::new(),rationale:"Import retained API authoring snapshot; human publication approval required".to_owned()}).await?;
            revisions.insert(key.clone(), revision);
        }
        Ok(ImportedSkills {
            source_id: source.clone(),
            snapshot_id: snapshot,
            revisions,
        })
    }
}
