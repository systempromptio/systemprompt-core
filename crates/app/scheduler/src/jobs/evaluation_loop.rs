//! Scheduled auto-improve evaluation pass: sample recent production AI
//! requests, judge them against a rubric, and replay failures with the
//! judge's repair hint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use systemprompt_ai::AiService;
use systemprompt_analytics::AnalyticsAiSessionProvider;
use systemprompt_database::DbPool;
use systemprompt_evaluation::{EvaluationService, RunRequest, SampleFilter, TriggerSource};
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::McpToolProvider;
use systemprompt_models::ai::DynAiProvider;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::error::SchedulerError;

const DEFAULT_SAMPLE_SIZE: i64 = 20;
const DEFAULT_WINDOW_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy)]
pub struct EvaluationLoopJob;

#[async_trait]
impl Job for EvaluationLoopJob {
    fn name(&self) -> &'static str {
        "evaluation_loop"
    }

    fn description(&self) -> &'static str {
        "Judges a sample of recent AI requests against a rubric and replays failures with repair \
         hints (parameters: sample_size, window_hours, rubric, judge_provider, judge_model, \
         budget_microdollars)"
    }

    fn schedule(&self) -> &'static str {
        "0 0 4 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );
        let app_context = Arc::clone(
            ctx.app_context::<Arc<AppContext>>()
                .ok_or_else(|| SchedulerError::missing_context("AppContext"))?,
        );

        let ai_service = build_ai_service(&db_pool, &app_context)?;
        let ai_provider: DynAiProvider = Arc::<AiService>::clone(&ai_service);
        let evaluation = EvaluationService::new(&db_pool, ai_provider).map_err(internal)?;

        let sample_size = ctx
            .get_parameter_parsed::<i64>("sample_size")?
            .unwrap_or(DEFAULT_SAMPLE_SIZE);
        let window_hours = ctx
            .get_parameter_parsed::<i64>("window_hours")?
            .unwrap_or(DEFAULT_WINDOW_HOURS);

        let request = RunRequest {
            judge_provider: ctx
                .get_parameter("judge_provider")
                .cloned()
                .unwrap_or_else(|| ai_service.default_provider().to_owned()),
            judge_model: ctx
                .get_parameter("judge_model")
                .cloned()
                .unwrap_or_else(|| ai_service.default_model().to_owned()),
            rubric_name: ctx.get_parameter("rubric").cloned(),
            filter: SampleFilter::with_limit(sample_size)
                .since(Utc::now() - Duration::hours(window_hours)),
            budget_microdollars: ctx.get_parameter_parsed::<i64>("budget_microdollars")?,
            created_by: app_context.system_admin().id().clone(),
            trigger_source: TriggerSource::Scheduled,
        };

        let (run_id, report) = evaluation.run_judge(request).await.map_err(internal)?;

        info!(
            run_id = %run_id,
            scored = report.scored,
            failed = report.failed,
            replayed = report.replayed,
            repaired = report.repaired,
            judge_cost_microdollars = report.judge_cost_microdollars,
            "Job completed"
        );

        Ok(JobResult {
            success: true,
            message: Some(format!(
                "run {run_id}: scored {}, failed {}, repaired {}",
                report.scored, report.failed, report.repaired
            )),
            items_processed: Some(u64::from(report.scored)),
            items_failed: Some(u64::from(report.failed)),
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }
}

fn build_ai_service(
    db_pool: &DbPool,
    app_context: &Arc<AppContext>,
) -> Result<Arc<AiService>, SchedulerError> {
    let services_config = ConfigLoader::load().map_err(internal)?;
    let profile = systemprompt_config::ProfileBootstrap::get().map_err(internal)?;

    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(db_pool),
        app_context.mcp_registry().clone(),
        &services_config.ai.mcp.resilience,
    ));
    let session_provider = Arc::new(AnalyticsAiSessionProvider::new(db_pool)?);
    Ok(Arc::new(
        AiService::new(
            db_pool,
            &profile.providers,
            &services_config.ai,
            tool_provider,
            session_provider,
        )
        .map_err(internal)?,
    ))
}

fn internal(e: impl std::fmt::Display) -> SchedulerError {
    SchedulerError::Internal(e.to_string())
}

systemprompt_provider_contracts::submit_job!(&EvaluationLoopJob);
