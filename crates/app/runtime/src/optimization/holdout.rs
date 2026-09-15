//! Guided independent confirmation retains its matrix before explicit approval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::diagnostics::DiagnosticContext;
use super::{OptimizationError, SkillOptimizationOrchestrator};
use serde::{Deserialize, Serialize};
use systemprompt_evaluation::campaigns::diagnostics::{DiagnosticCode, DiagnosticStage};
use systemprompt_evaluation::campaigns::holdout::HoldoutProposal;
use systemprompt_evaluation::experiments::records::ExperimentStatus;
use systemprompt_evaluation::experiments::resources::{CaseContent, Partition, ResourceContent};
use systemprompt_evaluation::experiments::{ExperimentSpec, content_digest};
use systemprompt_evaluation::repository::experiments::{CampaignAvailability, CampaignExperiment};
use systemprompt_identifiers::{
    EvalCampaignId, EvalExperimentId, EvalHoldoutProposalId, EvalRevisionId, UserId,
};

/// Select a completed development run and a separately authored holdout
/// dataset.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrepareHoldout {
    pub development_experiment_id: EvalExperimentId,
    pub holdout_dataset_id: EvalRevisionId,
    pub idempotency_key: String,
}
/// Explicit confirmation binds human approval to the retained matrix digest.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmHoldout {
    pub spec_digest: String,
    pub confirm_independent_holdout: bool,
}
/// The proposal a confirmation targets, and who is approving it.
#[derive(Debug, Clone, Copy)]
pub struct HoldoutConfirmationTarget<'a> {
    pub actor: &'a UserId,
    pub campaign: &'a EvalCampaignId,
    pub id: &'a EvalHoldoutProposalId,
}
/// Reviewable proposal and current actual execution admission, without reserved
/// spend.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct HoldoutReview {
    pub proposal: HoldoutProposal,
    pub execution_availability: CampaignAvailability,
}
struct PartitionedCases {
    development: Vec<EvalRevisionId>,
    holdout: Vec<EvalRevisionId>,
}
fn case_digest(case: &CaseContent) -> Result<String, OptimizationError> {
    Ok(content_digest(&(
        &case.prompt,
        &case.expected_behavior,
        &case.fixtures,
        &case.assertions,
    ))?)
}
impl SkillOptimizationOrchestrator {
    pub async fn prepare_holdout(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
        input: &PrepareHoldout,
    ) -> Result<HoldoutReview, OptimizationError> {
        let result = self.prepare_holdout_inner(owner, campaign, input).await;
        let ctx = DiagnosticContext {
            owner,
            actor,
            campaign,
            key: &input.idempotency_key,
            stage: DiagnosticStage::Holdout,
        };
        match &result {
            Err(error) => self.retain_failure(&ctx, error).await?,
            Ok(review) if !review.execution_availability.admitted => {
                self.blocked(&ctx, DiagnosticCode::UnsupportedCapability)
                    .await?;
            },
            Ok(_) => {},
        }
        result
    }
    async fn prepare_holdout_inner(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        input: &PrepareHoldout,
    ) -> Result<HoldoutReview, OptimizationError> {
        let report = self
            .report(owner, campaign, &input.development_experiment_id)
            .await?;
        if !report.development.eligible {
            return Err(OptimizationError::Source("Complete a qualifying development comparison before independent holdout preparation".to_owned()));
        }
        let mut spec = self
            .completed_development_spec(owner, &input.development_experiment_id)
            .await?;
        let cases = self
            .partition_cases(owner, &spec.cases, &input.holdout_dataset_id)
            .await?;
        let counts = (
            i32::try_from(cases.development.len()).unwrap_or(i32::MAX),
            i32::try_from(cases.holdout.len()).unwrap_or(i32::MAX),
        );
        let minimum = i32::try_from(report.campaign.policy.minimum_pairs).unwrap_or(i32::MAX);
        if counts.0 < minimum || counts.1 < minimum {
            return Err(OptimizationError::Source(
                "Both partitions must meet the unchanged campaign minimum paired-case threshold"
                    .to_owned(),
            ));
        }
        self.freeze_holdout_spec(owner, &mut spec, cases).await?;
        let proposal = self
            .evaluations
            .campaigns
            .propose_holdout(
                owner,
                systemprompt_evaluation::campaigns::holdout::HoldoutProposalRequest {
                    campaign,
                    development: &input.development_experiment_id,
                    key: &input.idempotency_key,
                    spec: &spec,
                    counts,
                },
            )
            .await?;
        Ok(HoldoutReview {
            execution_availability: self
                .evaluations
                .experiments
                .execution_availability(&proposal.spec),
            proposal,
        })
    }
    async fn completed_development_spec(
        &self,
        owner: &UserId,
        experiment: &EvalExperimentId,
    ) -> Result<ExperimentSpec, OptimizationError> {
        let detail = self.evaluations.experiments.get(owner, experiment).await?;
        if detail.experiment.status != ExperimentStatus::Completed
            || detail.experiment.accounting.reserved != 0
            || detail.experiment.accounting.frozen
        {
            return Err(OptimizationError::Source(
                "Development execution and accounting must be complete".to_owned(),
            ));
        }
        Ok(detail.experiment.spec)
    }
    async fn partition_cases(
        &self,
        owner: &UserId,
        development_cases: &[EvalRevisionId],
        holdout_dataset: &EvalRevisionId,
    ) -> Result<PartitionedCases, OptimizationError> {
        let mut development = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for id in development_cases {
            if let ResourceContent::Case(case) = self.evaluations.revisions.get(owner, id).await?
                && case.partition == Partition::Development
            {
                seen.insert(case_digest(&case)?);
                development.push(id.clone());
            }
        }
        let ResourceContent::Dataset(ids) = self
            .evaluations
            .revisions
            .get(owner, holdout_dataset)
            .await?
        else {
            return Err(OptimizationError::Source(
                "Select a retained holdout dataset".to_owned(),
            ));
        };
        let mut holdout = Vec::new();
        for id in ids {
            let ResourceContent::Case(case) = self.evaluations.revisions.get(owner, &id).await?
            else {
                return Err(OptimizationError::Source(
                    "Holdout dataset must contain cases".to_owned(),
                ));
            };
            if case.partition != Partition::Holdout {
                continue;
            }
            if !seen.insert(case_digest(&case)?) {
                return Err(OptimizationError::Source(
                    "Holdout must not duplicate development or another holdout case".to_owned(),
                ));
            }
            holdout.push(id);
        }
        Ok(PartitionedCases {
            development,
            holdout,
        })
    }
    async fn freeze_holdout_spec(
        &self,
        owner: &UserId,
        spec: &mut ExperimentSpec,
        cases: PartitionedCases,
    ) -> Result<(), OptimizationError> {
        let PartitionedCases {
            mut development,
            holdout,
        } = cases;
        development.extend(holdout);
        spec.cases = development;
        let dataset = ResourceContent::Dataset(spec.cases.clone());
        let dataset_id = self
            .evaluations
            .revisions
            .create(
                owner,
                &format!("holdout:{}", content_digest(&dataset)?),
                &dataset,
            )
            .await?;
        spec.dataset = Some(dataset_id);
        spec.claim_independent_improvement = true;
        let frozen = spec.frozen.as_mut().ok_or_else(|| {
            OptimizationError::Source("Holdout requires frozen execution settings".to_owned())
        })?;
        frozen.dataset_digest = content_digest(&dataset)?;
        let executions = u64::try_from(spec.cases.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(2)
            .saturating_mul(u64::from(spec.repetitions));
        spec.budget_microdollars = frozen.cost_envelope.maximum_microdollars(executions)?;
        spec.validate()?;
        Ok(())
    }
    pub async fn confirm_holdout(
        &self,
        owner: &UserId,
        target: &HoldoutConfirmationTarget<'_>,
        input: &ConfirmHoldout,
    ) -> Result<HoldoutProposal, OptimizationError> {
        let ctx = DiagnosticContext {
            owner,
            actor: target.actor,
            campaign: target.campaign,
            key: target.id.as_str(),
            stage: DiagnosticStage::Holdout,
        };
        let result = self.confirm_holdout_inner(&ctx, target.id, input).await;
        if let Err(error) = &result {
            self.retain_failure(&ctx, error).await?;
        }
        result
    }
    async fn confirm_holdout_inner(
        &self,
        ctx: &DiagnosticContext<'_>,
        id: &EvalHoldoutProposalId,
        input: &ConfirmHoldout,
    ) -> Result<HoldoutProposal, OptimizationError> {
        if !input.confirm_independent_holdout {
            self.blocked(ctx, DiagnosticCode::InvalidInput).await?;
            return Err(OptimizationError::Source(
                "Explicit independent holdout confirmation is required".to_owned(),
            ));
        }
        let proposal = self
            .evaluations
            .campaigns
            .confirm_holdout(
                ctx.owner,
                systemprompt_evaluation::campaigns::holdout::HoldoutConfirmation {
                    actor: ctx.actor,
                    campaign: ctx.campaign,
                    id,
                    digest: &input.spec_digest,
                },
            )
            .await?;
        if proposal.experiment_id.is_some() {
            return Ok(proposal);
        }
        let experiment = self
            .launch(
                ctx.owner,
                ctx.actor,
                &CampaignExperiment {
                    campaign_id: ctx.campaign.clone(),
                    idempotency_key: format!("holdout:{}", proposal.id),
                    spec: proposal.spec,
                },
            )
            .await?;
        Ok(self
            .evaluations
            .campaigns
            .attach_holdout_run(ctx.owner, ctx.campaign, id, &experiment)
            .await?)
    }
}
