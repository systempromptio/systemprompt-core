//! Publication attestations are produced by application-level evaluation and
//! source verification. Repository publication verifies the retained binding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AssetDigest, ManagedError, ManagedRepository, Result};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    EvalCampaignId, EvalExperimentId, ManagedResourceId, ResourceRevisionId, UserId,
};

pub(super) async fn admit_improvement(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserId,
    request: &super::PublicationRequest,
    bundle_digest: Option<&AssetDigest>,
) -> Result<()> {
    if request.action != super::PublicationAction::PublishImprovement {
        return Ok(());
    }
    let experiment = request
        .comparison_evidence
        .experiment_id
        .as_ref()
        .ok_or_else(|| {
            ManagedError::Conflict("Improvement requires an attested experiment".to_owned())
        })?;
    let revision = request
        .revision_id
        .as_ref()
        .ok_or(ManagedError::Integrity)?;
    let digest = bundle_digest.ok_or(ManagedError::Integrity)?;
    let eligible = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_evaluation_attestations a JOIN managed_dependency_verifications v ON v.owner_id=a.owner_id AND v.revision_id=a.revision_id AND v.commit_sha=a.source_commit AND v.bundle_digest=a.bundle_digest WHERE a.owner_id=$1 AND a.resource_id=$2 AND a.revision_id=$3 AND a.bundle_digest=$4 AND a.experiment_id=$5)", owner.as_str(), request.resource_id.as_str(), revision.as_str(), digest.as_str(), experiment.as_str()).fetch_one(&mut **tx).await?.unwrap_or(false);
    if !eligible {
        return Err(ManagedError::Conflict(
            "No eligible evaluation and source attestation exists for this exact candidate"
                .to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluationAttestation {
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub bundle_digest: AssetDigest,
    pub experiment_id: EvalExperimentId,
    pub campaign_id: EvalCampaignId,
    pub evidence_digest: AssetDigest,
    pub source_commit: String,
}

impl ManagedRepository {
    pub async fn attest_evaluation(
        &self,
        owner: &UserId,
        actor: &UserId,
        evidence: &EvaluationAttestation,
    ) -> Result<()> {
        self.require_verified_git_content(owner, &evidence.revision_id, &evidence.source_commit)
            .await?;
        if !matches!(evidence.source_commit.len(), 40 | 64)
            || !evidence
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(super::error::invalid(
                "Attestation requires an exact source commit",
            ));
        }
        if self.revision_resource(owner, &evidence.revision_id).await? != evidence.resource_id
            || self
                .get_revision_bundle(owner, &evidence.revision_id)
                .await?
                .digest()?
                != evidence.bundle_digest
        {
            return Err(ManagedError::Integrity);
        }
        sqlx::query!("INSERT INTO managed_evaluation_attestations(owner_id,resource_id,revision_id,bundle_digest,experiment_id,campaign_id,evidence_digest,source_commit,attested_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING", owner.as_str(), evidence.resource_id.as_str(), evidence.revision_id.as_str(), evidence.bundle_digest.as_str(), evidence.experiment_id.as_str(), evidence.campaign_id.as_str(), evidence.evidence_digest.as_str(), &evidence.source_commit, actor.as_str()).execute(&self.pool).await?;
        let matched = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_evaluation_attestations WHERE owner_id=$1 AND resource_id=$2 AND revision_id=$3 AND experiment_id=$4 AND bundle_digest=$5 AND evidence_digest=$6 AND source_commit=$7 AND campaign_id=$8)", owner.as_str(), evidence.resource_id.as_str(), evidence.revision_id.as_str(), evidence.experiment_id.as_str(), evidence.bundle_digest.as_str(), evidence.evidence_digest.as_str(), &evidence.source_commit, evidence.campaign_id.as_str()).fetch_one(&self.pool).await?.unwrap_or(false);
        if !matched {
            return Err(ManagedError::Conflict(
                "Attestation conflicts with retained evidence".to_owned(),
            ));
        }
        Ok(())
    }
}
