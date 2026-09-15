//! Bounded development iterations reuse frozen execution settings and the
//! campaign budget. Holdout cases never enter automatic improvement runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_evaluation::campaigns::diagnostics::{DiagnosticCode, DiagnosticStage};
use systemprompt_evaluation::campaigns::suggestions::RetainedSuggestion;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_evaluation::experiments::records::{ExperimentDetail, ExperimentStatus};
use systemprompt_evaluation::experiments::resources::{Partition, ResourceContent};
use systemprompt_evaluation::experiments::{ExperimentSpec, Objective};
use systemprompt_evaluation::models::CampaignStatus;
use systemprompt_evaluation::repository::experiments::{
    CampaignExperiment, ManagedWorkspaceRegistration,
};
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, ResourceRevisionId, UserId};

use super::diagnostics::DiagnosticContext;
use super::{OptimizationError, SkillOptimizationOrchestrator};

impl SkillOptimizationOrchestrator {
    pub(super) async fn launch_inner(
        &self,
        owner: &UserId,
        actor: &UserId,
        input: &CampaignExperiment,
    ) -> Result<EvalExperimentId, OptimizationError> {
        self.validate_campaign_spec(owner, &input.campaign_id, &input.spec)
            .await?;
        Ok(self
            .evaluations
            .experiments
            .create_for_campaign(owner, actor, input)
            .await?)
    }


    pub async fn attach(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign_id: &EvalCampaignId,
        experiment_id: &EvalExperimentId,
    ) -> Result<(), OptimizationError> {
        let detail = self
            .evaluations
            .experiments
            .get(owner, experiment_id)
            .await?;
        self.validate_campaign_spec(owner, campaign_id, &detail.experiment.spec)
            .await?;
        Ok(self
            .evaluations
            .campaigns
            .attach_experiment(owner, actor, campaign_id, experiment_id)
            .await?)
    }

    async fn validate_campaign_spec(
        &self,
        owner: &UserId,
        campaign_id: &EvalCampaignId,
        spec: &ExperimentSpec,
    ) -> Result<(), OptimizationError> {
        let campaign = self.evaluations.campaigns.get(owner, campaign_id).await?;
        let baseline = self
            .managed
            .get_revision_bundle(owner, &campaign.policy.baseline_revision_id)
            .await?;
        if spec.variants.len() != 2
            || spec.variants[0].skill_bundle_digest != baseline.digest()?.as_str()
        {
            return Err(OptimizationError::Source(
                "Campaign runs require the retained baseline and one candidate".to_owned(),
            ));
        }
        let workspace = self
            .evaluations
            .evidence
            .get_managed_workspace(owner, &spec.variants[1].skill_bundle_digest)
            .await?;
        if self
            .managed
            .revision_resource(owner, &workspace.managed_revision_id)
            .await?
            != campaign.policy.resource_id
        {
            return Err(OptimizationError::Source(
                "Candidate must belong to the campaign resource".to_owned(),
            ));
        }
        Ok(())
    }

    pub(super) async fn advance_inner(
        &self,
        owner: &UserId,
        actor: &UserId,
        id: &EvalCampaignId,
    ) -> Result<Option<EvalExperimentId>, OptimizationError> {
        let campaign = self.evaluations.campaigns.get(owner, id).await?;
        let experiments = self
            .evaluations
            .campaigns
            .list_experiments(owner, id)
            .await?;
        if campaign.status != CampaignStatus::Active || !campaign.policy.automatic {
            return Ok(None);
        }
        let ctx = DiagnosticContext {
            owner,
            actor,
            campaign: id,
            key: "automatic",
            stage: DiagnosticStage::AutomaticFollowup,
        };
        if experiments.len() >= campaign.policy.maximum_iterations as usize {
            self.blocked(&ctx, DiagnosticCode::IterationLimit).await?;
            return Ok(None);
        }
        let Some(previous) = experiments.last() else {
            self.blocked(&ctx, DiagnosticCode::MissingTemplate).await?;
            return Ok(None);
        };
        let Some((detail, suggestion)) = self.select_followup(&ctx, previous).await? else {
            return Ok(None);
        };
        let revision = self
            .apply_suggestion(
                owner,
                &campaign.policy,
                &detail.experiment.spec.variants[1],
                &suggestion,
            )
            .await?;
        let digest = self.register_workspace(owner, &revision).await?;
        let spec = self
            .development_followup_spec(owner, detail.experiment.spec, digest, &campaign.policy)
            .await?;
        let input = CampaignExperiment {
            campaign_id: id.clone(),
            idempotency_key: format!("suggestion:{}", suggestion.id),
            spec,
        };
        Ok(Some(self.launch(owner, actor, &input).await?))
    }

    async fn select_followup(
        &self,
        ctx: &DiagnosticContext<'_>,
        previous: &EvalExperimentId,
    ) -> Result<Option<(ExperimentDetail, RetainedSuggestion)>, OptimizationError> {
        let detail = self
            .evaluations
            .experiments
            .get(ctx.owner, previous)
            .await?;
        if detail.experiment.status != ExperimentStatus::Completed
            || detail.experiment.spec.variants.len() != 2
            || detail.experiment.spec.claim_independent_improvement
        {
            return Ok(None);
        }
        let suggestions = self
            .evaluations
            .lifecycle
            .list_suggestions(ctx.owner, previous)
            .await?;
        let Some(suggestion) = suggestions
            .into_iter()
            .find(|suggestion| suggestion.status == "draft")
        else {
            self.blocked(ctx, DiagnosticCode::MissingSuggestion).await?;
            return Ok(None);
        };
        if !self
            .evaluations
            .experiments
            .execution_availability(&detail.experiment.spec)
            .admitted
        {
            self.blocked(ctx, DiagnosticCode::UnsupportedCapability)
                .await?;
            return Ok(None);
        }
        Ok(Some((detail, suggestion)))
    }

    async fn development_followup_spec(
        &self,
        owner: &UserId,
        mut spec: ExperimentSpec,
        candidate_digest: String,
        policy: &CampaignPolicy,
    ) -> Result<ExperimentSpec, OptimizationError> {
        let mut development = Vec::new();
        for case in &spec.cases {
            if let ResourceContent::Case(content) = self.revisions.get(owner, case).await?
                && content.partition == Partition::Development
            {
                development.push(case.clone());
            }
        }
        if development.is_empty() {
            return Err(OptimizationError::Source(
                "No development cases available for automatic iteration".to_owned(),
            ));
        }
        spec.cases = development;
        spec.claim_independent_improvement = false;
        spec.variants[1].skill_bundle_digest = candidate_digest;
        spec.objective = match policy.objective {
            OptimizationObjective::Quality => Objective::Quality,
            OptimizationObjective::Tokens => Objective::Tokens,
            OptimizationObjective::Cost => Objective::Cost,
            OptimizationObjective::Latency => Objective::Latency,
        };
        Ok(spec)
    }


    pub async fn register_workspace(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<String, OptimizationError> {
        let bundle = self.managed.get_revision_bundle(owner, revision).await?;
        let digest = bundle.digest()?;
        match self
            .evaluations
            .evidence
            .get_managed_workspace(owner, digest.as_str())
            .await
        {
            Ok(_) => return Ok(digest.as_str().to_owned()),
            Err(systemprompt_evaluation::EvaluationError::ResourceNotFound(_)) => {},
            Err(error) => return Err(error.into()),
        }
        let files: Vec<_> = bundle
            .revisions
            .values()
            .flat_map(|manifest| manifest.files.values())
            .collect();
        let bytes = files
            .iter()
            .try_fold(0usize, |sum, file| {
                usize::try_from(file.bytes)
                    .ok()
                    .and_then(|bytes| sum.checked_add(bytes))
            })
            .ok_or_else(|| OptimizationError::Source("Bundle size overflow".to_owned()))?;
        self.evaluations
            .evidence
            .register_managed_workspace(
                owner,
                &ManagedWorkspaceRegistration {
                    managed_revision_id: revision,
                    publication_generation: None,
                    manifest: &bundle,
                    expected_digest: digest.as_str(),
                    file_count: files.len(),
                    byte_count: bytes,
                },
            )
            .await?;
        Ok(digest.as_str().to_owned())
    }
}
