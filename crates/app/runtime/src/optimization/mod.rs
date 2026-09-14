//! Cross-domain source verification and evaluation attestation. The managed
//! domain owns content; evaluation owns measurements; this layer binds them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_evaluation::campaigns::report::{self, CampaignReport};
use systemprompt_evaluation::repository::experiments::{
    EvaluationRepositories, RevisionRepository,
};
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, ResourceRevisionId, UserId};
use systemprompt_marketplace::managed::evaluation::EvaluationAttestation;
use systemprompt_marketplace::managed::{AssetDigest, ManagedRepository};
mod candidate;
mod capture;
mod diagnostics;
pub mod git_sources;
pub mod holdout;
pub mod inventory;
mod iteration;

#[derive(Debug, thiserror::Error)]
pub enum OptimizationError {
    #[error(transparent)]
    Evaluation(#[from] systemprompt_evaluation::EvaluationError),
    #[error(transparent)]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Source verification failed: {0}")]
    Source(String),
}

#[derive(Debug, Clone)]
pub struct SkillOptimizationOrchestrator {
    managed: ManagedRepository,
    evaluations: EvaluationRepositories,
    revisions: RevisionRepository,
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
    pub const fn new(
        managed: ManagedRepository,
        evaluations: EvaluationRepositories,
        revisions: RevisionRepository,
    ) -> Self {
        Self {
            managed,
            evaluations,
            revisions,
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
            &self.revisions,
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
            evidence_digest: AssetDigest::of(&serde_json::to_vec(&(
                &report.campaign.policy,
                &report.experiment_id,
                &report.baseline_bundle_digest,
                &report.candidate_bundle_digest,
                &report.development,
                &report.holdout,
            ))?),
            source_commit,
        };
        self.managed
            .attest_evaluation(owner, actor, &evidence)
            .await?;
        Ok(evidence)
    }
}
