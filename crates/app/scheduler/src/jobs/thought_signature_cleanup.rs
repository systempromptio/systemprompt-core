//! Hourly job deleting expired Gemini thought signatures from
//! `ai_gateway_thought_signatures`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_ai::repository::AiThoughtSignatureRepository;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::error::SchedulerError;

#[derive(Debug, Clone, Copy)]
pub struct ThoughtSignatureCleanupJob;

#[async_trait]
impl Job for ThoughtSignatureCleanupJob {
    fn name(&self) -> &'static str {
        "thought_signature_cleanup"
    }

    fn description(&self) -> &'static str {
        "Deletes expired gateway thought signatures"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );

        let deleted = AiThoughtSignatureRepository::new(&db_pool)
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(e.to_string()))?
            .cleanup_expired()
            .await
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(e.to_string()))?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        if deleted > 0 {
            info!(
                deleted = deleted,
                duration_ms = duration_ms,
                "Thought signature cleanup completed"
            );
        }

        Ok(JobResult::success()
            .with_stats(deleted, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ThoughtSignatureCleanupJob);
