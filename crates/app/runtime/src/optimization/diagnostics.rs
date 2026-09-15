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

#[derive(Debug, Clone, Copy)]
pub(super) struct DiagnosticContext<'a> {
    pub owner: &'a UserId,
    pub actor: &'a UserId,
    pub campaign: &'a EvalCampaignId,
    pub key: &'a str,
    pub stage: DiagnosticStage,
}

impl SkillOptimizationOrchestrator {
    pub(super) async fn blocked(
        &self,
        ctx: &DiagnosticContext<'_>,
        code: DiagnosticCode,
    ) -> Result<(), OptimizationError> {
        let operation = format!(
            "{}:{}",
            ctx.campaign,
            systemprompt_evaluation::experiments::content_digest(&ctx.key)?
        );
        self.evaluations
            .campaigns
            .record_diagnostic(
                ctx.owner,
                systemprompt_evaluation::campaigns::diagnostics::DiagnosticRecord {
                    actor: ctx.actor,
                    campaign: Some(ctx.campaign),
                    operation: &operation,
                    stage: ctx.stage,
                    code,
                },
            )
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
        ctx: &DiagnosticContext<'_>,
        error: &OptimizationError,
    ) -> Result<(), OptimizationError> {
        let code = match error {
            OptimizationError::Evaluation(error) => DiagnosticCode::from_error(error),
            _ => DiagnosticCode::InvalidInput,
        };
        self.blocked(ctx, code).await
    }
    pub async fn launch(
        &self,
        owner: &UserId,
        actor: &UserId,
        input: &CampaignExperiment,
    ) -> Result<EvalExperimentId, OptimizationError> {
        let result = self.launch_inner(owner, actor, input).await;
        if let Err(error) = &result {
            let ctx = DiagnosticContext {
                owner,
                actor,
                campaign: &input.campaign_id,
                key: &input.idempotency_key,
                stage: DiagnosticStage::Launch,
            };
            self.retain_failure(&ctx, error).await?;
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
            let ctx = DiagnosticContext {
                owner,
                actor,
                campaign,
                key: "automatic",
                stage: DiagnosticStage::AutomaticFollowup,
            };
            self.retain_failure(&ctx, error).await?;
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
            let ctx = DiagnosticContext {
                owner,
                actor: owner,
                campaign,
                key: experiment.as_str(),
                stage: DiagnosticStage::Report,
            };
            self.retain_failure(&ctx, error).await?;
        } else {
            self.resolve_blocked(owner, campaign, experiment.as_str())
                .await?;
        }
        result
    }
}
