//! Immutable worker inputs retrieved only through a live, owner-scoped lease.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{EvidenceRepository, ExecutionLease, WorkerRecord};
use crate::Result;
use crate::experiments::execution::FrozenWorkspace;
use crate::experiments::records::ExecutionRecord;
use crate::experiments::resources::ResourceContent;
use crate::experiments::{ExperimentSpec, conflict, content_digest, invalid};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAssignment {
    pub execution: ExecutionRecord,
    pub spec: ExperimentSpec,
    pub spec_digest: String,
    pub case: ResourceContent,
    pub case_digest: String,
    pub rubric: ResourceContent,
    pub rubric_digest: String,
    pub skill_bundle: FrozenWorkspace,
    pub configuration: FrozenWorkspace,
}

#[derive(Debug, Clone)]
pub struct AssignmentRepository {
    pool: PgPool,
    evidence: EvidenceRepository,
}

impl AssignmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            evidence: EvidenceRepository::new(pool.clone()),
            pool,
        }
    }

    pub async fn get(
        &self,
        worker: &WorkerRecord,
        lease: &ExecutionLease,
    ) -> Result<ExecutionAssignment> {
        let row = sqlx::query!(
            r#"SELECT to_jsonb(x) AS "execution!: Json<ExecutionRecord>", e.spec AS "spec!: Json<ExperimentSpec>", e.spec_digest, c.content AS "case!: Json<ResourceContent>", c.digest AS case_digest, r.content AS "rubric!: Json<ResourceContent>", r.digest AS rubric_digest FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_workers w ON w.id=x.lease_owner AND w.owner_id=e.owner_id JOIN eval_resource_revisions c ON c.id=x.case_revision_id AND c.owner_id=e.owner_id JOIN eval_resource_revisions r ON r.id=e.spec->>'rubric' AND r.owner_id=e.owner_id WHERE e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND w.id=$5 AND w.environment=$6 AND w.enabled AND w.expires_at>NOW() AND e.status='running' AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()"#,
            worker.owner_id.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token, worker.id.as_str(), worker.environment
        ).fetch_optional(&self.pool).await?.ok_or_else(|| conflict("Assignment requires a live, owned worker lease"))?;
        let spec = row.spec.0;
        spec.validate()?;
        if content_digest(&spec)? != row.spec_digest
            || content_digest(&row.case.0)? != row.case_digest
            || content_digest(&row.rubric.0)? != row.rubric_digest
        {
            return Err(invalid("Assignment revision digest mismatch"));
        }
        row.case.0.validate()?;
        row.rubric.0.validate()?;
        if !matches!(row.case.0, ResourceContent::Case(_))
            || !matches!(row.rubric.0, ResourceContent::Rubric(_))
            || !spec.cases.contains(&row.execution.0.case_revision_id)
        {
            return Err(invalid(
                "Assignment resource types or case membership mismatch",
            ));
        }
        let index = usize::try_from(row.execution.0.variant_index).map_err(|error| {
            invalid(&format!(
                "Invalid assignment variant index {}: {error}",
                row.execution.0.variant_index
            ))
        })?;
        let variant = spec
            .variants
            .get(index)
            .ok_or_else(|| invalid("Assignment variant unavailable"))?;
        let skill_bundle = self
            .evidence
            .get_workspace(&worker.owner_id, &variant.skill_bundle_digest)
            .await?;
        let configuration = self
            .evidence
            .get_workspace(&worker.owner_id, &variant.configuration_digest)
            .await?;
        Ok(ExecutionAssignment {
            execution: row.execution.0,
            spec,
            spec_digest: row.spec_digest,
            case: row.case.0,
            case_digest: row.case_digest,
            rubric: row.rubric.0,
            rubric_digest: row.rubric_digest,
            skill_bundle,
            configuration,
        })
    }
}
