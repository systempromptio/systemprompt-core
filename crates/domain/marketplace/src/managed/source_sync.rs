//! Hook-free Git synchronization into immutable incoming revisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ManagedResourceId, ManagedSourceId, ResourceRevisionId, SourceSnapshotId, UserId,
    WithdrawalProposalId,
};

use super::{
    AssetDigest, AssetFile, ManagedError, ManagedRepository, NewRevision, Result, RevisionFiles,
    SnapshotProvenance, SourceSpec,
};

#[path = "source_sync_git.rs"]
mod git_import;
use git_import::{import_tree, resolve_ref};

const IMPORTER_VERSION: &str = "managed-git-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSyncRequest {
    pub source_id: ManagedSourceId,
    pub resource_id: ManagedResourceId,
    pub upstream_root: String,
    pub upstream_base_revision_id: Option<ResourceRevisionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum GitSyncResult {
    Incoming {
        snapshot_id: SourceSnapshotId,
        revision_id: ResourceRevisionId,
        commit: String,
    },
    WithdrawalProposed {
        snapshot_id: SourceSnapshotId,
        proposal_id: WithdrawalProposalId,
        commit: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WithdrawalProposal {
    pub id: String,
    pub resource_id: String,
    pub snapshot_id: String,
    pub reason: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub decided_by: Option<String>,
    pub decided_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ManagedRepository {
    pub async fn list_withdrawal_proposals(
        &self,
        owner: &UserId,
    ) -> Result<Vec<WithdrawalProposal>> {
        Ok(sqlx::query_as!(WithdrawalProposal, "SELECT id,resource_id,snapshot_id,reason,status,created_at,decided_by,decided_at FROM managed_withdrawal_proposals WHERE owner_id=$1 ORDER BY created_at DESC LIMIT 100",
            owner.as_str()).fetch_all(&self.pool).await?)
    }

    pub async fn decide_withdrawal_proposal(
        &self,
        owner: &UserId,
        actor: &UserId,
        proposal: &WithdrawalProposalId,
        approved: bool,
    ) -> Result<()> {
        let status = if approved { "approved" } else { "rejected" };
        let changed = sqlx::query!("UPDATE managed_withdrawal_proposals SET status=$4,decided_by=$2,decided_at=NOW() WHERE id=$1 AND owner_id=$3 AND status='pending'",
            proposal.as_str(), actor.as_str(), owner.as_str(), status).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(ManagedError::Conflict(
                "Withdrawal proposal is stale or unavailable".to_owned(),
            ));
        }
        Ok(())
    }

    pub async fn sync_git_source(
        &self,
        owner: &UserId,
        request: &GitSyncRequest,
    ) -> Result<GitSyncResult> {
        self.sync_git_source_with_credential(owner, request, None)
            .await
    }

    pub async fn sync_git_source_with_credential(
        &self,
        owner: &UserId,
        request: &GitSyncRequest,
        credential: Option<&str>,
    ) -> Result<GitSyncResult> {
        super::assets::validate_path(&request.upstream_root)?;
        let spec = self.get_source(owner, &request.source_id).await?;
        let SourceSpec::Git {
            repository,
            reference,
            subdirectory,
            credential_reference: _,
        } = spec
        else {
            return Err(super::error::invalid("Source is not Git-backed"));
        };
        if credential.is_some_and(str::is_empty) {
            return Err(super::error::invalid("Resolved Git credential is empty"));
        }
        let commit = resolve_ref(&repository, &reference, credential)?;
        let temp = std::env::temp_dir().join(format!(
            "systemprompt-managed-{}",
            ManagedSourceId::generate().as_str()
        ));
        std::fs::create_dir(&temp)?;
        let import = import_tree(
            &temp,
            &repository,
            &commit,
            subdirectory.as_deref(),
            &request.upstream_root,
            credential,
        );
        let cleanup = std::fs::remove_dir_all(&temp);
        let files = import?;
        cleanup?;
        let tree_digest = AssetDigest::of(&serde_jcs::to_vec(&files)?);
        let snapshot_id = self
            .capture_snapshot(
                owner,
                &request.source_id,
                &SnapshotProvenance {
                    source_kind: "git".to_owned(),
                    commit: Some(commit.clone()),
                    tree_digest,
                    importer_version: IMPORTER_VERSION.to_owned(),
                },
            )
            .await?;
        if files.0.is_empty() {
            let proposal_id = WithdrawalProposalId::generate();
            sqlx::query!("INSERT INTO managed_withdrawal_proposals(id,owner_id,resource_id,snapshot_id,reason) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,resource_id,snapshot_id) DO NOTHING",
                proposal_id.as_str(), owner.as_str(), request.resource_id.as_str(), snapshot_id.as_str(), "Upstream Git tree removed the managed resource").execute(&self.pool).await?;
            let stored = sqlx::query_scalar!("SELECT id FROM managed_withdrawal_proposals WHERE owner_id=$1 AND resource_id=$2 AND snapshot_id=$3",
                owner.as_str(), request.resource_id.as_str(), snapshot_id.as_str()).fetch_one(&self.pool).await?;
            return Ok(GitSyncResult::WithdrawalProposed {
                snapshot_id,
                proposal_id: WithdrawalProposalId::new(stored),
                commit,
            });
        }
        let revision_id = self
            .create_revision(
                owner,
                &NewRevision {
                    resource_id: request.resource_id.clone(),
                    snapshot_id: snapshot_id.clone(),
                    parent_id: request.upstream_base_revision_id.clone(),
                    files,
                    dependencies: BTreeMap::new(),
                    rationale: format!("Incoming synchronization from Git commit {commit}"),
                },
            )
            .await?;
        Ok(GitSyncResult::Incoming {
            snapshot_id,
            revision_id,
            commit,
        })
    }
}
