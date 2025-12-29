mod helpers;
mod prompt;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult};

use crate::repository::EvaluationRepository;
use crate::services::NoopToolProvider;
use crate::AiService;
use helpers::evaluate_single_conversation;

#[derive(Debug, Clone, Copy)]
pub struct EvaluateConversationsJob;

#[async_trait]
impl Job for EvaluateConversationsJob {
    fn name(&self) -> &'static str {
        "evaluate_conversations"
    }

    fn description(&self) -> &'static str {
        "Evaluates completed AI conversations for quality and goal achievement"
    }

    fn schedule(&self) -> &'static str {
        "0 */5 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow!("DbPool not available in job context"))?,
        );

        let app_context = Arc::clone(
            ctx.app_context::<Arc<AppContext>>()
                .ok_or_else(|| anyhow!("AppContext not available in job context"))?,
        );

        tracing::info!(batch_size = 50, "Job started");

        let repository = EvaluationRepository::new(&db_pool)?;
        let conversations = repository.get_unevaluated_conversations(50).await?;

        if conversations.is_empty() {
            tracing::debug!("No unevaluated conversations found");
            return Ok(JobResult::success()
                .with_message("No unevaluated conversations found")
                .with_duration(start_time.elapsed().as_millis() as u64));
        }

        tracing::debug!(count = conversations.len(), "Conversations found");

        let services_config = ConfigLoader::load()?;
        let tool_provider = Arc::new(NoopToolProvider::new());
        let ai_service = AiService::new(&app_context, &services_config.ai, tool_provider)?;

        let mut success_count = 0u64;
        let mut error_count = 0u64;

        for conversation in &conversations {
            let context_id = conversation
                .get("context_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing context_id"))?;

            match evaluate_single_conversation(context_id, conversation, &ai_service, &repository)
                .await
            {
                Ok(()) => {
                    success_count += 1;
                },
                Err(e) => {
                    error_count += 1;
                    tracing::error!(context_id = %context_id, error = %e, "Evaluation failed");
                },
            }
        }

        tracing::info!(
            succeeded = success_count,
            failed = error_count,
            total_evaluated = success_count + error_count,
            duration_ms = start_time.elapsed().as_millis(),
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(success_count, error_count)
            .with_duration(start_time.elapsed().as_millis() as u64))
    }
}

systemprompt_traits::submit_job!(&EvaluateConversationsJob);
