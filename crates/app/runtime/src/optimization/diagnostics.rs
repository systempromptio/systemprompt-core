//! Retain safe actionable failures independently of HTTP and scheduler
//! lifetimes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::{OptimizationError, SkillOptimizationOrchestrator};
use systemprompt_evaluation::campaigns::diagnostics::{DiagnosticCode, DiagnosticStage};
use systemprompt_evaluation::campaigns::report::CampaignReport;
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, UserId};

impl SkillOptimizationOrchestrator {
    pub(super) async fn blocked(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
        key: &str,
        stage: DiagnosticStage,
        code: DiagnosticCode,
    ) -> Result<(), OptimizationError> {
        let operation = format!(
            "{}:{}",
            campaign,
            systemprompt_evaluation::experiments::content_digest(&key)?
        );
        self.evaluations
            .campaigns
            .record_diagnostic(owner, actor, Some(campaign), &operation, stage, code)
            .await?;
        Ok(())
    }
    async fn resolve_blocked(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        key: &str,
    ) -> Result<(), OptimizationError> {
        let operation = format!(
            "{}:{}",
            campaign,
            systemprompt_evaluation::experiments::content_digest(&key)?
        );
        self.evaluations
            .campaigns
            .resolve_diagnostics(owner, campaign, &operation)
            .await?;
        Ok(())
    }
    pub(super) async fn retain_failure(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
        key: &str,
        stage: DiagnosticStage,
        error: &OptimizationError,
    ) -> Result<(), OptimizationError> {
        let code = match error {
            OptimizationError::Evaluation(error) => DiagnosticCode::from_error(error),
            _ => DiagnosticCode::InvalidInput,
        };
        self.blocked(owner, actor, campaign, key, stage, code).await
    }
    pub async fn launch(
        &self,
        owner: &UserId,
        actor: &UserId,
        input: &CampaignExperiment,
    ) -> Result<EvalExperimentId, OptimizationError> {
        let result = self.launch_inner(owner, actor, input).await;
        if let Err(error) = &result {
            self.retain_failure(
                owner,
                actor,
                &input.campaign_id,
                &input.idempotency_key,
                DiagnosticStage::Launch,
                error,
            )
            .await?;
        } else {
            self.resolve_blocked(owner, &input.campaign_id, &input.idempotency_key)
                .await?;
        }
        result
    }
    pub async fn advance(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
    ) -> Result<Option<EvalExperimentId>, OptimizationError> {
        let result = self.advance_inner(owner, actor, campaign).await;
        if let Err(error) = &result {
            self.retain_failure(
                owner,
                actor,
                campaign,
                "automatic",
                DiagnosticStage::AutomaticFollowup,
                error,
            )
            .await?;
        } else if matches!(&result, Ok(Some(_))) {
            self.resolve_blocked(owner, campaign, "automatic").await?;
        }
        result
    }
    pub async fn report(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        experiment: &EvalExperimentId,
    ) -> Result<CampaignReport, OptimizationError> {
        let result = self.report_inner(owner, campaign, experiment).await;
        if let Err(error) = &result {
            self.retain_failure(
                owner,
                owner,
                campaign,
                experiment.as_str(),
                DiagnosticStage::Report,
                error,
            )
            .await?;
        } else {
            self.resolve_blocked(owner, campaign, experiment.as_str())
                .await?;
        }
        result
    }
}
