//! Durable three-way reconciliation for managed and incoming revisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ManagedReconciliationId, ManagedResourceId, ResourceRevisionId, UserId,
};

use super::{ManagedError, ManagedRepository, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationRequest {
    pub resource_id: ManagedResourceId,
    pub upstream_base_revision_id: ResourceRevisionId,
    pub managed_candidate_revision_id: ResourceRevisionId,
    pub incoming_revision_id: ResourceRevisionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationConflict {
    pub path: String,
    pub base_digest: Option<String>,
    pub candidate_digest: Option<String>,
    pub incoming_digest: Option<String>,
    pub resolution: Option<ConflictResolution>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    Candidate,
    Incoming,
    Manual,
    Delete,
}

impl ConflictResolution {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Incoming => "incoming",
            Self::Manual => "manual",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRecord {
    pub id: ManagedReconciliationId,
    pub status: String,
    pub conflicts: Vec<ReconciliationConflict>,
    pub resolved_revision_id: Option<ResourceRevisionId>,
}

impl ManagedRepository {
    pub async fn begin_reconciliation(
        &self,
        owner: &UserId,
        request: &ReconciliationRequest,
    ) -> Result<ReconciliationRecord> {
        let revisions = [
            &request.upstream_base_revision_id,
            &request.managed_candidate_revision_id,
            &request.incoming_revision_id,
        ];
        for revision in revisions {
            let found = sqlx::query_scalar!(
                "SELECT id FROM managed_revisions WHERE owner_id=$1 AND resource_id=$2 AND id=$3",
                owner.as_str(),
                request.resource_id.as_str(),
                revision.as_str()
            )
            .fetch_optional(&self.pool)
            .await?;
            if found.is_none() {
                return Err(ManagedError::Unavailable);
            }
        }
        let base = self
            .get_revision_files(owner, &request.upstream_base_revision_id)
            .await?;
        let candidate = self
            .get_revision_files(owner, &request.managed_candidate_revision_id)
            .await?;
        let incoming = self
            .get_revision_files(owner, &request.incoming_revision_id)
            .await?;
        let mut paths = std::collections::BTreeSet::new();
        paths.extend(base.0.keys().cloned());
        paths.extend(candidate.0.keys().cloned());
        paths.extend(incoming.0.keys().cloned());
        let mut conflicts = Vec::new();
        for path in paths {
            let digest = |files: &super::RevisionFiles| {
                files
                    .0
                    .get(&path)
                    .map(|file| super::AssetDigest::of(&file.bytes).as_str().to_owned())
            };
            let b = digest(&base);
            let c = digest(&candidate);
            let i = digest(&incoming);
            if c != b && i != b && c != i {
                conflicts.push(ReconciliationConflict {
                    path,
                    base_digest: b,
                    candidate_digest: c,
                    incoming_digest: i,
                    resolution: None,
                });
            }
        }
        let id = ManagedReconciliationId::generate();
        let mut tx = self.pool.begin().await?;
        sqlx::query!("INSERT INTO managed_reconciliations(id,owner_id,resource_id,upstream_base_revision_id,managed_candidate_revision_id,incoming_revision_id) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(owner_id,resource_id,managed_candidate_revision_id,incoming_revision_id) DO NOTHING",
            id.as_str(), owner.as_str(), request.resource_id.as_str(), request.upstream_base_revision_id.as_str(), request.managed_candidate_revision_id.as_str(), request.incoming_revision_id.as_str()).execute(&mut *tx).await?;
        let stored = sqlx::query_scalar!("SELECT id FROM managed_reconciliations WHERE owner_id=$1 AND resource_id=$2 AND managed_candidate_revision_id=$3 AND incoming_revision_id=$4",
            owner.as_str(), request.resource_id.as_str(), request.managed_candidate_revision_id.as_str(), request.incoming_revision_id.as_str()).fetch_one(&mut *tx).await?;
        for conflict in &conflicts {
            sqlx::query!("INSERT INTO managed_reconciliation_conflicts(reconciliation_id,path,base_digest,candidate_digest,incoming_digest) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",
                &stored, &conflict.path, conflict.base_digest.as_deref(), conflict.candidate_digest.as_deref(), conflict.incoming_digest.as_deref()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(ReconciliationRecord {
            id: ManagedReconciliationId::new(stored),
            status: "open".to_owned(),
            conflicts,
            resolved_revision_id: None,
        })
    }

    pub async fn resolve_reconciliation_conflict(
        &self,
        owner: &UserId,
        id: &ManagedReconciliationId,
        path: &str,
        resolution: ConflictResolution,
        resolved_digest: Option<&str>,
    ) -> Result<()> {
        super::assets::validate_path(path)?;
        match (resolution, resolved_digest) {
            (ConflictResolution::Manual, Some(digest))
                if digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) => {},
            (ConflictResolution::Manual, _) => {
                return Err(super::error::invalid(
                    "Manual reconciliation requires an exact lowercase SHA-256 digest",
                ));
            },
            (_, None) => {},
            (_, Some(_)) => {
                return Err(super::error::invalid(
                    "Only manual reconciliation accepts a resolved digest",
                ));
            },
        }
        let changed = sqlx::query!("UPDATE managed_reconciliation_conflicts c SET resolution=$3,resolved_digest=$4 FROM managed_reconciliations r WHERE c.reconciliation_id=r.id AND r.id=$1 AND r.owner_id=$2 AND c.path=$5 AND r.status='open'",
            id.as_str(), owner.as_str(), resolution.as_str(), resolved_digest, path).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(ManagedError::Unavailable);
        }
        Ok(())
    }

    pub async fn complete_reconciliation(
        &self,
        owner: &UserId,
        actor: &UserId,
        id: &ManagedReconciliationId,
        revision: &ResourceRevisionId,
    ) -> Result<()> {
        let row = sqlx::query!("SELECT resource_id,upstream_base_revision_id,managed_candidate_revision_id,incoming_revision_id FROM managed_reconciliations WHERE id=$1 AND owner_id=$2 AND status='open'",
            id.as_str(), owner.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        let conflicts = sqlx::query!("SELECT path,resolution,resolved_digest FROM managed_reconciliation_conflicts WHERE reconciliation_id=$1 ORDER BY path",
            id.as_str()).fetch_all(&self.pool).await?;
        if conflicts
            .iter()
            .any(|conflict| conflict.resolution.is_none())
        {
            return Err(ManagedError::Conflict(
                "Reconciliation has unresolved conflicts".to_owned(),
            ));
        }
        let base = self
            .get_revision_files(
                owner,
                &ResourceRevisionId::new(row.upstream_base_revision_id),
            )
            .await?;
        let candidate = self
            .get_revision_files(
                owner,
                &ResourceRevisionId::new(row.managed_candidate_revision_id),
            )
            .await?;
        let incoming = self
            .get_revision_files(owner, &ResourceRevisionId::new(row.incoming_revision_id))
            .await?;
        let resolved = self.get_revision_files(owner, revision).await?;
        let belongs = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_revisions WHERE owner_id=$1 AND resource_id=$2 AND id=$3)",
            owner.as_str(), row.resource_id, revision.as_str()).fetch_one(&self.pool).await?.unwrap_or(false);
        if !belongs {
            return Err(ManagedError::Unavailable);
        }

        let mut paths = std::collections::BTreeSet::new();
        paths.extend(base.0.keys().cloned());
        paths.extend(candidate.0.keys().cloned());
        paths.extend(incoming.0.keys().cloned());
        paths.extend(resolved.0.keys().cloned());
        for path in paths {
            let base_file = base.0.get(&path);
            let candidate_file = candidate.0.get(&path);
            let incoming_file = incoming.0.get(&path);
            let expected = if same_file(candidate_file, incoming_file) {
                candidate_file
            } else if same_file(candidate_file, base_file) {
                incoming_file
            } else if same_file(incoming_file, base_file) {
                candidate_file
            } else {
                let conflict = conflicts
                    .iter()
                    .find(|conflict| conflict.path == path)
                    .ok_or_else(|| {
                        ManagedError::Conflict("Three-way conflict record is missing".to_owned())
                    })?;
                match conflict.resolution.as_deref() {
                    Some("candidate") => candidate_file,
                    Some("incoming") => incoming_file,
                    Some("delete") => None,
                    Some("manual") => {
                        let actual = resolved.0.get(&path).ok_or_else(|| {
                            ManagedError::Conflict(
                                "Manual resolution content is missing".to_owned(),
                            )
                        })?;
                        let digest = super::AssetDigest::of(&actual.bytes);
                        if conflict.resolved_digest.as_deref() != Some(digest.as_str()) {
                            return Err(ManagedError::Conflict(
                                "Manual resolution digest does not match supplied content"
                                    .to_owned(),
                            ));
                        }
                        Some(actual)
                    },
                    _ => {
                        return Err(ManagedError::Conflict(
                            "Invalid reconciliation resolution".to_owned(),
                        ));
                    },
                }
            };
            if !same_file(resolved.0.get(&path), expected) {
                return Err(ManagedError::Conflict(format!(
                    "Resolved revision differs from the recorded merge at {path}"
                )));
            }
        }
        let changed = sqlx::query!("UPDATE managed_reconciliations r SET status='resolved',resolved_revision_id=$3,resolved_by=$4,resolved_at=NOW() WHERE r.id=$1 AND r.owner_id=$2 AND r.status='open' AND NOT EXISTS(SELECT 1 FROM managed_reconciliation_conflicts c WHERE c.reconciliation_id=r.id AND c.resolution IS NULL)",
            id.as_str(), owner.as_str(), revision.as_str(), actor.as_str()).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(ManagedError::Conflict(
                "Reconciliation has unresolved conflicts".to_owned(),
            ));
        }
        Ok(())
    }
}

fn same_file(left: Option<&super::AssetFile>, right: Option<&super::AssetFile>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.bytes == right.bytes
                && left.media_type == right.media_type
                && left.executable == right.executable
        },
        _ => false,
    }
}
