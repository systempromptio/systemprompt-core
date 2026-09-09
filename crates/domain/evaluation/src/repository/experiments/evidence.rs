//! Immutable workspaces and fenced execution evidence, scoped to their owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ExecutionLease;
use crate::Result;
use crate::experiments::execution::{ExecutionEvidence, FrozenWorkspace};
use crate::experiments::{VariantSpec, conflict, content_digest, invalid, missing};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{EvalExecutionId, UserId};

#[derive(Debug, Clone)]
pub struct EvidenceRepository {
    pool: PgPool,
}

impl EvidenceRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn save_workspace(
        &self,
        owner: &UserId,
        workspace: &FrozenWorkspace,
    ) -> Result<String> {
        let digest = workspace.digest()?;
        sqlx::query!(
            "INSERT INTO eval_frozen_workspaces(owner_id,digest,content) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
            owner.as_str(), digest, Json(workspace) as _
        ).execute(&self.pool).await?;
        Ok(digest)
    }

    pub async fn get_workspace(&self, owner: &UserId, digest: &str) -> Result<FrozenWorkspace> {
        let workspace = sqlx::query_scalar!(
            r#"SELECT content AS "content!: Json<FrozenWorkspace>" FROM eval_frozen_workspaces WHERE owner_id=$1 AND digest=$2"#,
            owner.as_str(), digest
        ).fetch_optional(&self.pool).await?
            .ok_or_else(|| missing("Frozen workspace unavailable in this scope"))?.0;
        if workspace.digest()? != digest {
            return Err(invalid("Frozen workspace digest mismatch"));
        }
        Ok(workspace)
    }

    pub async fn submit(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        evidence: &ExecutionEvidence,
        artifacts: &FrozenWorkspace,
    ) -> Result<()> {
        evidence.validate()?;
        validate_artifacts(evidence, artifacts)?;
        if evidence.execution_id != lease.execution_id
            || evidence.fencing_token != lease.fencing_token
        {
            return Err(conflict("Evidence does not match its execution lease"));
        }
        let digest = content_digest(evidence)?;
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let execution = sqlx::query!(
            r#"SELECT x.status,(x.lease_expires_at>NOW() AND x.deadline_at>NOW()) AS live,e.spec->'variants'->x.variant_index AS "variant!: Json<VariantSpec>" FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND e.status='running'"#,
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token
        ).fetch_optional(&mut *tx).await?.ok_or_else(|| conflict("Foreign or cancelled execution lease"))?;
        if execution.status != "running" || execution.live != Some(true) {
            return Err(conflict("Expired execution lease"));
        }
        validate_variant(evidence, &execution.variant.0)?;
        let mut recorded = sqlx::query_scalar!(
            "SELECT m.request_id FROM eval_request_reservations m JOIN ai_requests r ON r.id=m.request_id WHERE m.execution_id=$1 AND r.user_id=$2",
            lease.execution_id.as_str(), owner.as_str()
        ).fetch_all(&mut *tx).await?;
        let mut submitted: Vec<_> = evidence
            .requests
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        recorded.sort();
        submitted.sort();
        if recorded != submitted {
            return Err(conflict(
                "Evidence request IDs differ from the server audit trail",
            ));
        }
        let existing = sqlx::query_scalar!(
            "SELECT digest FROM eval_execution_evidence WHERE execution_id=$1",
            lease.execution_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing != digest {
                return Err(conflict("Execution evidence is immutable"));
            }
            return Ok(());
        }
        sqlx::query!(
            "INSERT INTO eval_execution_evidence(execution_id,fencing_token,digest,manifest) VALUES($1,$2,$3,$4)",
            lease.execution_id.as_str(), lease.fencing_token, digest, Json(evidence) as _
        ).execute(&mut *tx).await?;
        sqlx::query!(
            "INSERT INTO eval_execution_artifacts(execution_id,content) VALUES($1,$2)",
            lease.execution_id.as_str(),
            Json(artifacts) as _
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_artifacts(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
    ) -> Result<FrozenWorkspace> {
        Ok(sqlx::query_scalar!(
            r#"SELECT a.content AS "content!: Json<FrozenWorkspace>" FROM eval_execution_artifacts a JOIN eval_executions x ON x.id=a.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2"#,
            owner.as_str(), execution.as_str()
        ).fetch_optional(&self.pool).await?.ok_or_else(|| missing("Artifacts unavailable in this scope"))?.0)
    }

    pub async fn get(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
    ) -> Result<ExecutionEvidence> {
        Ok(sqlx::query_scalar!(
            r#"SELECT m.manifest AS "manifest!: Json<ExecutionEvidence>" FROM eval_execution_evidence m JOIN eval_executions x ON x.id=m.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2"#,
            owner.as_str(), execution.as_str()
        ).fetch_optional(&self.pool).await?
            .ok_or_else(|| missing("Execution evidence unavailable in this scope"))?.0)
    }
}

fn validate_artifacts(evidence: &ExecutionEvidence, artifacts: &FrozenWorkspace) -> Result<()> {
    artifacts.validate()?;
    if artifacts.files.len() != evidence.artifacts.len() {
        return Err(invalid("Artifact payload differs from its manifest"));
    }
    for artifact in &evidence.artifacts {
        let bytes = artifacts
            .files
            .get(&artifact.relative_path)
            .ok_or_else(|| invalid("Manifest artifact payload missing"))?
            .as_bytes();
        if bytes.len() as u64 != artifact.bytes
            || hex::encode(Sha256::digest(bytes)) != artifact.sha256
        {
            return Err(invalid(
                "Artifact content does not match its declared hash or size",
            ));
        }
    }
    Ok(())
}

fn validate_variant(evidence: &ExecutionEvidence, variant: &VariantSpec) -> Result<()> {
    if evidence.candidate_bundle_digest != variant.skill_bundle_digest
        || evidence.installed_bundle_digest != variant.skill_bundle_digest
        || evidence.capabilities.image_digest != variant.worker_image_digest
        || evidence.capabilities.client != variant.client
        || evidence.capabilities.client_version != variant.client_version
    {
        return Err(conflict(
            "Evidence differs from the frozen experiment variant",
        ));
    }
    Ok(())
}
