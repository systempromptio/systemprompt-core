//! Cross-domain source verification and evaluation attestation. The managed
//! domain owns content; evaluation owns measurements; this layer binds them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_evaluation::campaigns::CampaignPolicy;
use systemprompt_evaluation::campaigns::comparison::ComparisonDecision;
use systemprompt_evaluation::campaigns::report::{self, CampaignReport};
use systemprompt_evaluation::repository::experiments::EvaluationRepositories;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, ResourceRevisionId, UserId};
use systemprompt_marketplace::managed::evaluation::EvaluationAttestation;
use systemprompt_marketplace::managed::{AssetDigest, ManagedRepository};
mod candidate;
mod capture;
mod diagnostics;
pub mod git_sources;
pub mod holdout;
mod holdout_partition;
pub mod inventory;
mod iteration;

#[derive(Debug, thiserror::Error)]
pub enum OptimizationError {
    #[error(transparent)]
    Evaluation(#[from] systemprompt_evaluation::EvaluationError),
    #[error(transparent)]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),
    #[error(transparent)]
    Bundle(#[from] systemprompt_models::managed::RevisionBundleError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Source verification failed: {0}")]
    Source(String),
}

#[derive(Debug, Clone)]
pub struct SkillOptimizationOrchestrator {
    managed: ManagedRepository,
    evaluations: EvaluationRepositories,
}

/// The facts an evaluation attestation commits to, hashed as canonical JSON
/// (RFC 8785) so the digest does not depend on field order.
#[derive(Debug, Serialize)]
pub struct EvaluationEvidence<'a> {
    pub policy: &'a CampaignPolicy,
    pub experiment_id: &'a EvalExperimentId,
    pub baseline_bundle_digest: &'a str,
    pub candidate_bundle_digest: &'a str,
    pub development: &'a ComparisonDecision,
    pub holdout: &'a ComparisonDecision,
}

impl EvaluationEvidence<'_> {
    pub fn digest(&self) -> Result<AssetDigest, OptimizationError> {
        Ok(AssetDigest::of(&serde_jcs::to_vec(self)?))
    }
}

impl<'a> From<&'a CampaignReport> for EvaluationEvidence<'a> {
    fn from(report: &'a CampaignReport) -> Self {
        Self {
            policy: &report.campaign.policy,
            experiment_id: &report.experiment_id,
            baseline_bundle_digest: &report.baseline_bundle_digest,
            candidate_bundle_digest: &report.candidate_bundle_digest,
            development: &report.development,
            holdout: &report.holdout,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceAcceptance {
    pub campaign_id: EvalCampaignId,
    pub experiment_id: EvalExperimentId,
    pub evaluated_revision_id: ResourceRevisionId,
    pub committed_revision_id: ResourceRevisionId,
    pub source_commit: String,
}

impl SkillOptimizationOrchestrator {
    pub const fn new(managed: ManagedRepository, evaluations: EvaluationRepositories) -> Self {
        Self {
            managed,
            evaluations,
        }
    }

    pub(super) async fn report_inner(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        experiment: &EvalExperimentId,
    ) -> Result<CampaignReport, OptimizationError> {
        let report = report::build(
            &self.evaluations,
            &self.evaluations.revisions,
            owner,
            campaign,
            experiment,
        )
        .await?;
        let baseline = self
            .managed
            .get_revision_bundle(owner, &report.campaign.policy.baseline_revision_id)
            .await?;
        if baseline.digest()?.as_str() != report.baseline_bundle_digest {
            return Err(OptimizationError::Source(
                "Experiment baseline differs from the campaign baseline".to_owned(),
            ));
        }
        Ok(report)
    }

    pub async fn accept_source(
        &self,
        owner: &UserId,
        actor: &UserId,
        input: &SourceAcceptance,
    ) -> Result<EvaluationAttestation, OptimizationError> {
        let report = self
            .report(owner, &input.campaign_id, &input.experiment_id)
            .await?;
        if !report.eligible_for_publication {
            return Err(OptimizationError::Source(
                "Experiment does not establish an eligible improvement".to_owned(),
            ));
        }
        let resource = &report.campaign.policy.resource_id;
        for revision in [&input.evaluated_revision_id, &input.committed_revision_id] {
            if self.managed.revision_resource(owner, revision).await? != *resource {
                return Err(OptimizationError::Source(
                    "Source revisions must belong to the campaign resource".to_owned(),
                ));
            }
        }
        let evaluated = self
            .managed
            .get_revision_bundle(owner, &input.evaluated_revision_id)
            .await?;
        let committed = self
            .managed
            .get_revision_bundle(owner, &input.committed_revision_id)
            .await?;
        if evaluated.digest()?.as_str() != report.candidate_bundle_digest
            || evaluated.content_digest()? != committed.content_digest()?
        {
            return Err(OptimizationError::Source("Committed content or dependencies differ from the evaluated candidate; reevaluation required".to_owned()));
        }
        self.managed
            .require_verified_git_content(owner, &input.committed_revision_id, &input.source_commit)
            .await?;
        let source_commit = input.source_commit.clone();
        let evidence = EvaluationAttestation {
            resource_id: resource.clone(),
            revision_id: input.committed_revision_id.clone(),
            bundle_digest: committed.digest()?,
            experiment_id: input.experiment_id.clone(),
            campaign_id: input.campaign_id.clone(),
            evidence_digest: EvaluationEvidence::from(&report).digest()?,
            source_commit,
        };
        self.managed
            .attest_evaluation(owner, actor, &evidence)
            .await?;
        Ok(evidence)
    }
}
