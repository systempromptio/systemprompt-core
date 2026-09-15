//! Immutable workspaces and fenced execution evidence, scoped to their owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ExecutionLease;
use crate::Result;
use crate::experiments::execution::{EvidenceArchive, ExecutionEvidence};
use crate::experiments::{VariantSpec, conflict, content_digest, invalid, missing};

use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{AiRequestId, EvalExecutionId, ResourceRevisionId, UserId};
use systemprompt_models::managed::RevisionBundle;
use systemprompt_traits::DynAiRequestTrace;

mod validation;
use validation::{ManagedAsset, managed_assets, validate_artifacts, validate_variant};

#[derive(Debug, Clone, Copy)]
pub struct ManagedWorkspaceRegistration<'a> {
    pub managed_revision_id: &'a ResourceRevisionId,
    pub publication_generation: Option<i64>,
    pub manifest: &'a RevisionBundle,
    pub expected_digest: &'a str,
    pub file_count: usize,
    pub byte_count: usize,
}

#[derive(Clone)]
pub struct EvidenceRepository {
    pool: PgPool,
    trace: DynAiRequestTrace,
}

impl std::fmt::Debug for EvidenceRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceRepository").finish_non_exhaustive()
    }
}

impl EvidenceRepository {
    pub const fn new(pool: PgPool, trace: DynAiRequestTrace) -> Self {
        Self { pool, trace }
    }

    pub async fn register_managed_workspace(
        &self,
        owner: &UserId,
        registration: &ManagedWorkspaceRegistration<'_>,
    ) -> Result<()> {
        let ManagedWorkspaceRegistration {
            managed_revision_id,
            publication_generation,
            manifest,
            expected_digest,
            file_count,
            byte_count,
        } = *registration;
        if content_digest(manifest)? != expected_digest
            || file_count > 256
            || byte_count > 8 * 1024 * 1024
        {
            return Err(invalid(
                "Managed workspace projection failed digest or size verification",
            ));
        }
        let assets = managed_assets(manifest)?;
        let manifest = serde_json::to_value(manifest)?;
        let expanded_bytes = assets
            .iter()
            .try_fold(0usize, |total, asset| {
                total.checked_add(asset.content.len())
            })
            .ok_or_else(|| invalid("Managed workspace asset size overflow"))?;
        if assets.len() != file_count || expanded_bytes != byte_count {
            return Err(invalid(
                "Managed workspace asset counts differ from the verified projection",
            ));
        }
        let file_count = i32::try_from(file_count)
            .map_err(|error| invalid(&format!("Managed file count overflow: {error}")))?;
        let byte_count = i64::try_from(byte_count)
            .map_err(|error| invalid(&format!("Managed byte count overflow: {error}")))?;
        let mut tx = self.pool.begin().await?;
        sqlx::query!("INSERT INTO eval_managed_workspace_projections(owner_id,digest,managed_revision_id,publication_generation,manifest,verified_file_count,verified_byte_count) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(owner_id,digest) DO NOTHING",
            owner.as_str(), expected_digest, managed_revision_id.as_str(), publication_generation, manifest, file_count, byte_count).execute(&mut *tx).await?;
        for asset in assets {
            sqlx::query!("INSERT INTO eval_managed_workspace_assets(owner_id,workspace_digest,path,asset_digest,content,executable) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(owner_id,workspace_digest,path) DO NOTHING",
                owner.as_str(), expected_digest, asset.path, asset.digest, asset.content, asset.executable).execute(&mut *tx).await?;
        }
        let verified = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_managed_workspace_projections p WHERE p.owner_id=$1 AND p.digest=$2 AND p.managed_revision_id=$3 AND p.publication_generation IS NOT DISTINCT FROM $4 AND p.manifest=$5 AND p.verified_file_count=$6 AND p.verified_byte_count=$7 AND p.verified_file_count=(SELECT count(*) FROM eval_managed_workspace_assets a WHERE a.owner_id=p.owner_id AND a.workspace_digest=p.digest) AND p.verified_byte_count=(SELECT COALESCE(sum(octet_length(a.content)),0) FROM eval_managed_workspace_assets a WHERE a.owner_id=p.owner_id AND a.workspace_digest=p.digest) AND NOT EXISTS(SELECT 1 FROM eval_managed_workspace_assets a WHERE a.owner_id=p.owner_id AND a.workspace_digest=p.digest AND a.asset_digest<>encode(digest(a.content,'sha256'),'hex')))",
            owner.as_str(), expected_digest, managed_revision_id.as_str(), publication_generation, manifest, file_count, byte_count).fetch_one(&mut *tx).await?.unwrap_or(false);
        if !verified {
            return Err(conflict(
                "Managed workspace registration conflicts with retained content",
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_managed_workspace(
        &self,
        owner: &UserId,
        digest: &str,
    ) -> Result<super::ManagedWorkspaceReference> {
        let row = sqlx::query!("SELECT managed_revision_id,publication_generation,manifest,verified_file_count,verified_byte_count FROM eval_managed_workspace_projections WHERE owner_id=$1 AND digest=$2",
            owner.as_str(), digest).fetch_optional(&self.pool).await?
            .ok_or_else(|| missing("Managed workspace projection unavailable in this scope"))?;
        let managed_revision_id = row.managed_revision_id;
        if managed_revision_id.starts_with("legacy-archive:") {
            return Err(missing(
                "Legacy workspace was retained as a verified archive and must be replaced by a managed bundle before execution",
            ));
        }
        if content_digest(&row.manifest)? != digest {
            return Err(invalid("Managed workspace projection digest mismatch"));
        }
        let manifest: RevisionBundle = serde_json::from_value(row.manifest).map_err(|error| {
            invalid(&format!(
                "Managed workspace projection is malformed: {error}"
            ))
        })?;
        let mut expected = managed_assets(&manifest)?;
        let stored = sqlx::query!("SELECT path,asset_digest,content,executable FROM eval_managed_workspace_assets WHERE owner_id=$1 AND workspace_digest=$2 ORDER BY path",
            owner.as_str(), digest).fetch_all(&self.pool).await?;
        expected.sort_by(|left, right| left.path.cmp(&right.path));
        let actual = stored
            .into_iter()
            .map(|asset| ManagedAsset {
                path: asset.path,
                digest: asset.asset_digest,
                content: asset.content,
                executable: asset.executable,
            })
            .collect::<Vec<_>>();
        let expected_bytes = expected
            .iter()
            .try_fold(0usize, |total, asset| {
                total.checked_add(asset.content.len())
            })
            .ok_or_else(|| invalid("Managed workspace asset size overflow"))?;
        if actual != expected
            || usize::try_from(row.verified_file_count).ok() != Some(expected.len())
            || usize::try_from(row.verified_byte_count).ok() != Some(expected_bytes)
        {
            return Err(invalid(
                "Managed workspace retained assets failed integrity verification",
            ));
        }
        Ok(super::ManagedWorkspaceReference {
            managed_revision_id: ResourceRevisionId::new(managed_revision_id),
            digest: digest.to_owned(),
            publication_generation: row.publication_generation,
            manifest,
        })
    }

    pub async fn submit(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        evidence: &ExecutionEvidence,
        artifacts: &EvidenceArchive,
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
        let reserved: Vec<AiRequestId> = sqlx::query_scalar!(
            "SELECT request_id FROM eval_request_reservations WHERE execution_id=$1",
            lease.execution_id.as_str()
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(AiRequestId::new)
        .collect();
        let mut recorded: Vec<AiRequestId> = self
            .trace
            .list_usage(owner, &reserved)
            .await?
            .into_iter()
            .map(|usage| usage.request_id)
            .collect();
        let mut submitted: Vec<AiRequestId> = evidence.requests.clone();
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
    ) -> Result<EvidenceArchive> {
        Ok(sqlx::query_scalar!(
            r#"SELECT a.content AS "content!: Json<EvidenceArchive>" FROM eval_execution_artifacts a JOIN eval_executions x ON x.id=a.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2"#,
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

    pub async fn list_request_ids(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
    ) -> Result<Vec<AiRequestId>> {
        Ok(sqlx::query_scalar!("SELECT m.request_id FROM eval_request_reservations m JOIN eval_executions x ON x.id=m.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 ORDER BY m.created_at",
            owner.as_str(), execution.as_str()).fetch_all(&self.pool).await?.into_iter().map(AiRequestId::new).collect())
    }

    pub async fn list_request_ids_by_traffic(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
        traffic_class: &str,
    ) -> Result<Vec<AiRequestId>> {
        if !matches!(
            traffic_class,
            "fixture" | "live_evaluation" | "suggestion" | "judge"
        ) {
            return Err(invalid("Unknown evaluation traffic class"));
        }
        Ok(sqlx::query_scalar!("SELECT m.request_id FROM eval_request_reservations m JOIN eval_executions x ON x.id=m.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND m.traffic_class=$3 ORDER BY m.created_at",
            owner.as_str(), execution.as_str(), traffic_class).fetch_all(&self.pool).await?.into_iter().map(AiRequestId::new).collect())
    }
}
