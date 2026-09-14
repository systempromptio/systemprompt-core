//! Guided independent confirmation retains its matrix before explicit approval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::{OptimizationError, SkillOptimizationOrchestrator};
use serde::{Deserialize, Serialize};
use systemprompt_evaluation::campaigns::diagnostics::{DiagnosticCode, DiagnosticStage};
use systemprompt_evaluation::campaigns::holdout::HoldoutProposal;
use systemprompt_evaluation::experiments::content_digest;
use systemprompt_evaluation::experiments::resources::{Partition, ResourceContent};
use systemprompt_evaluation::repository::experiments::{CampaignAvailability, CampaignExperiment};
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, EvalRevisionId, UserId};

/// Select a completed development run and a separately authored holdout
/// dataset.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareHoldout {
    pub development_experiment_id: EvalExperimentId,
    pub holdout_dataset_id: EvalRevisionId,
    pub idempotency_key: String,
}
/// Explicit confirmation binds human approval to the retained matrix digest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmHoldout {
    pub spec_digest: String,
    pub confirm_independent_holdout: bool,
}
/// Reviewable proposal and current actual execution admission, without reserved
/// spend.
#[derive(Debug, Clone, Serialize)]
pub struct HoldoutReview {
    pub proposal: HoldoutProposal,
    pub execution_availability: CampaignAvailability,
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
        match &result {
            Err(error) => {
                self.retain_failure(
                    owner,
                    actor,
                    campaign,
                    &input.idempotency_key,
                    DiagnosticStage::Holdout,
                    error,
                )
                .await?
            },
            Ok(review) if !review.execution_availability.admitted => {
                self.blocked(
                    owner,
                    actor,
                    campaign,
                    &input.idempotency_key,
                    DiagnosticStage::Holdout,
                    DiagnosticCode::UnsupportedCapability,
                )
                .await?
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
        let detail = self
            .evaluations
            .experiments
            .get(owner, &input.development_experiment_id)
            .await?;
        if detail.experiment.status
            != systemprompt_evaluation::experiments::records::ExperimentStatus::Completed
            || detail.experiment.accounting.reserved != 0
            || detail.experiment.accounting.frozen
        {
            return Err(OptimizationError::Source(
                "Development execution and accounting must be complete".to_owned(),
            ));
        }
        let mut spec = detail.experiment.spec;
        let mut development = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for id in &spec.cases {
            if let ResourceContent::Case(case) = self.revisions.get(owner, id).await? {
                if case.partition == Partition::Development {
                    seen.insert(content_digest(&(
                        &case.prompt,
                        &case.expected_behavior,
                        &case.fixtures,
                        &case.assertions,
                    ))?);
                    development.push(id.clone());
                }
            }
        }
        let ResourceContent::Dataset(ids) =
            self.revisions.get(owner, &input.holdout_dataset_id).await?
        else {
            return Err(OptimizationError::Source(
                "Select a retained holdout dataset".to_owned(),
            ));
        };
        let mut holdout = Vec::new();
        for id in ids {
            let ResourceContent::Case(case) = self.revisions.get(owner, &id).await? else {
                return Err(OptimizationError::Source(
                    "Holdout dataset must contain cases".to_owned(),
                ));
            };
            if case.partition != Partition::Holdout {
                continue;
            }
            if !seen.insert(content_digest(&(
                &case.prompt,
                &case.expected_behavior,
                &case.fixtures,
                &case.assertions,
            ))?) {
                return Err(OptimizationError::Source(
                    "Holdout must not duplicate development or another holdout case".to_owned(),
                ));
            }
            holdout.push(id);
        }
        let counts = (
            i32::try_from(development.len()).unwrap_or(i32::MAX),
            i32::try_from(holdout.len()).unwrap_or(i32::MAX),
        );
        if counts.0 < i32::try_from(report.campaign.policy.minimum_pairs).unwrap_or(i32::MAX)
            || counts.1 < i32::try_from(report.campaign.policy.minimum_pairs).unwrap_or(i32::MAX)
        {
            return Err(OptimizationError::Source(
                "Both partitions must meet the unchanged campaign minimum paired-case threshold"
                    .to_owned(),
            ));
        }
        development.extend(holdout);
        spec.cases = development;
        let dataset = ResourceContent::Dataset(spec.cases.clone());
        let dataset_id = self
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
        let proposal = self
            .evaluations
            .campaigns
            .propose_holdout(
                owner,
                campaign,
                &input.development_experiment_id,
                &input.idempotency_key,
                &spec,
                counts,
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
    pub async fn confirm_holdout(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
        id: &str,
        input: &ConfirmHoldout,
    ) -> Result<HoldoutProposal, OptimizationError> {
        let result = self
            .confirm_holdout_inner(owner, actor, campaign, id, input)
            .await;
        if let Err(error) = &result {
            self.retain_failure(owner, actor, campaign, id, DiagnosticStage::Holdout, error)
                .await?;
        }
        result
    }
    async fn confirm_holdout_inner(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
        id: &str,
        input: &ConfirmHoldout,
    ) -> Result<HoldoutProposal, OptimizationError> {
        if !input.confirm_independent_holdout {
            self.blocked(
                owner,
                actor,
                campaign,
                id,
                DiagnosticStage::Holdout,
                DiagnosticCode::InvalidInput,
            )
            .await?;
            return Err(OptimizationError::Source(
                "Explicit independent holdout confirmation is required".to_owned(),
            ));
        }
        let proposal = self
            .evaluations
            .campaigns
            .confirm_holdout(owner, actor, campaign, id, &input.spec_digest)
            .await?;
        if proposal.experiment_id.is_some() {
            return Ok(proposal);
        }
        let experiment = self
            .launch(
                owner,
                actor,
                &CampaignExperiment {
                    campaign_id: campaign.clone(),
                    idempotency_key: format!("holdout:{}", proposal.id),
                    spec: proposal.spec,
                },
            )
            .await?;
        Ok(self
            .evaluations
            .campaigns
            .attach_holdout_run(owner, campaign, id, &experiment)
            .await?)
    }
}
