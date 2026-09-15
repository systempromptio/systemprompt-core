//! Durable three-way reconciliation for managed and incoming revisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ManagedReconciliationId, ManagedResourceId, ResourceRevisionId, UserId,
};

mod merge;
use super::{ManagedError, ManagedRepository, Result};
use merge::{RecordedConflict, ThreeWay, same_file, verify_merge};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the request names four distinct revisions; the `_id` suffix is the typed-identifier convention"
)]
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
    fn parse(value: Option<&str>) -> Result<Option<Self>> {
        Ok(match value {
            None => None,
            Some("candidate") => Some(Self::Candidate),
            Some("incoming") => Some(Self::Incoming),
            Some("manual") => Some(Self::Manual),
            Some("delete") => Some(Self::Delete),
            Some(_) => return Err(ManagedError::Integrity),
        })
    }

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

fn detect_conflicts(
    base: &super::RevisionFiles,
    candidate: &super::RevisionFiles,
    incoming: &super::RevisionFiles,
) -> Vec<ReconciliationConflict> {
    let mut paths = std::collections::BTreeSet::new();
    paths.extend(base.0.keys().cloned());
    paths.extend(candidate.0.keys().cloned());
    paths.extend(incoming.0.keys().cloned());
    let digest = |files: &super::RevisionFiles, path: &str| {
        files
            .0
            .get(path)
            .map(|file| super::AssetDigest::of(&file.bytes).as_str().to_owned())
    };
    paths
        .into_iter()
        .filter(|path| {
            !same_file(candidate.0.get(path), base.0.get(path))
                && !same_file(incoming.0.get(path), base.0.get(path))
                && !same_file(candidate.0.get(path), incoming.0.get(path))
        })
        .map(|path| ReconciliationConflict {
            base_digest: digest(base, &path),
            candidate_digest: digest(candidate, &path),
            incoming_digest: digest(incoming, &path),
            path,
            resolution: None,
        })
        .collect()
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
        let conflicts = detect_conflicts(&base, &candidate, &incoming);
        let id = ManagedReconciliationId::generate();
        let mut tx = self.pool.begin().await?;
        sqlx::query!("INSERT INTO managed_reconciliations(id,owner_id,resource_id,upstream_base_revision_id,managed_candidate_revision_id,incoming_revision_id) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(owner_id,resource_id,managed_candidate_revision_id,incoming_revision_id) DO NOTHING",
            id.as_str(), owner.as_str(), request.resource_id.as_str(), request.upstream_base_revision_id.as_str(), request.managed_candidate_revision_id.as_str(), request.incoming_revision_id.as_str()).execute(&mut *tx).await?;
        let stored = sqlx::query!("SELECT id,upstream_base_revision_id,status,resolved_revision_id FROM managed_reconciliations WHERE owner_id=$1 AND resource_id=$2 AND managed_candidate_revision_id=$3 AND incoming_revision_id=$4 FOR UPDATE",
            owner.as_str(), request.resource_id.as_str(), request.managed_candidate_revision_id.as_str(), request.incoming_revision_id.as_str()).fetch_one(&mut *tx).await?;
        if stored.upstream_base_revision_id != request.upstream_base_revision_id.as_str() {
            return Err(ManagedError::Conflict(
                "Reconciliation retry changes the retained upstream base".to_owned(),
            ));
        }
        for conflict in &conflicts {
            sqlx::query!("INSERT INTO managed_reconciliation_conflicts(reconciliation_id,path,base_digest,candidate_digest,incoming_digest) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",
                &stored.id, &conflict.path, conflict.base_digest.as_deref(), conflict.candidate_digest.as_deref(), conflict.incoming_digest.as_deref()).execute(&mut *tx).await?;
        }
        let retained = sqlx::query!("SELECT path,base_digest,candidate_digest,incoming_digest,resolution FROM managed_reconciliation_conflicts WHERE reconciliation_id=$1 ORDER BY path",
            &stored.id).fetch_all(&mut *tx).await?;
        let conflicts = retained
            .into_iter()
            .map(|row| {
                Ok(ReconciliationConflict {
                    path: row.path,
                    base_digest: row.base_digest,
                    candidate_digest: row.candidate_digest,
                    incoming_digest: row.incoming_digest,
                    resolution: ConflictResolution::parse(row.resolution.as_deref())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok(ReconciliationRecord {
            id: ManagedReconciliationId::new(stored.id),
            status: stored.status,
            conflicts,
            resolved_revision_id: stored.resolved_revision_id.map(ResourceRevisionId::new),
        })
    }

    pub async fn resolve_reconciliation_conflict(
        &self,
        owner: &UserId,
        id: &ManagedReconciliationId,
        decision: &ConflictDecision<'_>,
    ) -> Result<()> {
        let ConflictDecision {
            path,
            resolution,
            resolved_digest,
        } = *decision;
        systemprompt_models::managed::validate_path(path)?;
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

        let recorded: Vec<RecordedConflict> = conflicts
            .into_iter()
            .map(|conflict| RecordedConflict {
                path: conflict.path,
                resolution: conflict.resolution,
                resolved_digest: conflict.resolved_digest,
            })
            .collect();
        verify_merge(
            &ThreeWay {
                base: &base,
                candidate: &candidate,
                incoming: &incoming,
            },
            &resolved,
            &recorded,
        )?;
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

#[derive(Debug, Clone, Copy)]
pub struct ConflictDecision<'a> {
    pub path: &'a str,
    pub resolution: ConflictResolution,
    pub resolved_digest: Option<&'a str>,
}
