//! Shared lookup and rendering helpers for the admin evals commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_ai::AiService;
use systemprompt_analytics::AnalyticsAiSessionProvider;
use systemprompt_evaluation::{EvaluationService, RunRequest, SampleFilter, TriggerSource};
use systemprompt_identifiers::UserId;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::McpToolProvider;
use systemprompt_models::ai::DynAiProvider;

use crate::context::CommandContext;

pub(super) struct EvalContext {
    pub evaluation: EvaluationService,
    pub default_provider: String,
    pub default_model: String,
    pub admin_id: UserId,
}

pub(super) async fn eval_context(ctx: &CommandContext) -> Result<EvalContext> {
    let app_context = ctx.app_context().await?;
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let profile = systemprompt_config::ProfileBootstrap::get()
        .context("Failed to access bootstrapped profile for provider registry")?;
    let db_pool = Arc::clone(app_context.db_pool());

    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(&db_pool),
        app_context.mcp_registry().clone(),
        &services_config.ai.mcp.resilience,
    ));
    let session_provider = Arc::new(
        AnalyticsAiSessionProvider::new(&db_pool)
            .context("Failed to create analytics session provider")?,
    );
    let ai_service = Arc::new(
        AiService::new(
            &db_pool,
            &profile.providers,
            &services_config.ai,
            tool_provider,
            session_provider,
        )
        .context("Failed to create AI service")?,
    );

    let default_provider = ai_service.default_provider().to_owned();
    let default_model = ai_service.default_model().to_owned();
    let ai_provider: DynAiProvider = ai_service;
    let evaluation = EvaluationService::new(&db_pool, ai_provider)
        .context("Failed to create evaluation service")?;

    Ok(EvalContext {
        evaluation,
        default_provider,
        default_model,
        admin_id: app_context.system_admin().id().clone(),
    })
}

#[derive(Debug, serde::Serialize)]
pub(super) struct ReportCard {
    pub run_id: String,
    pub scored: u32,
    pub failed: u32,
    pub replayed: u32,
    pub repaired: u32,
    pub judge_cost_microdollars: i64,
}

pub(super) fn report_card(
    run_id: &systemprompt_identifiers::EvalRunId,
    report: systemprompt_evaluation::LoopReport,
) -> ReportCard {
    ReportCard {
        run_id: run_id.as_str().to_owned(),
        scored: report.scored,
        failed: report.failed,
        replayed: report.replayed,
        repaired: report.repaired,
        judge_cost_microdollars: report.judge_cost_microdollars,
    }
}

#[derive(Debug, Default)]
pub(super) struct JudgeOptions {
    pub judge_provider: Option<String>,
    pub judge_model: Option<String>,
    pub rubric: Option<String>,
    pub budget_microdollars: Option<i64>,
}

pub(super) fn run_request(
    eval: &EvalContext,
    options: JudgeOptions,
    filter: SampleFilter,
) -> RunRequest {
    RunRequest {
        judge_provider: options
            .judge_provider
            .unwrap_or_else(|| eval.default_provider.clone()),
        judge_model: options
            .judge_model
            .unwrap_or_else(|| eval.default_model.clone()),
        rubric_name: options.rubric,
        filter,
        budget_microdollars: options.budget_microdollars,
        created_by: eval.admin_id.clone(),
        trigger_source: TriggerSource::Cli,
    }
}
