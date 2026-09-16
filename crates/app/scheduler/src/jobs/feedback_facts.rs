//! Restart-safe normalized analytics processing of the system admin's evidence
//! queue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::SchedulerError;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_analytics::feedback::FactsProcessingService;
use systemprompt_identifiers::AnalyticsWorkerId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderResult};

#[derive(Debug, Clone, Copy)]
pub struct FeedbackFactsJob;

#[async_trait]
impl Job for FeedbackFactsJob {
    fn name(&self) -> &'static str {
        "feedback_facts_processing"
    }
    fn description(&self) -> &'static str {
        "Processes committed normalized evidence with fenced leases and transactional checkpoints"
    }
    fn schedule(&self) -> &'static str {
        "*/5 * * * * *"
    }
    fn scope(&self) -> JobScope {
        JobScope::Node
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let app = ctx
            .app_context::<Arc<AppContext>>()
            .ok_or_else(|| SchedulerError::missing_context("AppContext"))?;
        let service = FactsProcessingService::new(app.feedback_facts_repository().as_ref().clone());
        let processed = service
            .drain(app.system_admin().id(), &AnalyticsWorkerId::generate(), 64)
            .await
            .map_err(SchedulerError::from)?;
        Ok(JobResult::success().with_stats(
            u64::try_from(processed)
                .map_err(|error| SchedulerError::config_error(error.to_string()))?,
            0,
        ))
    }
}

systemprompt_provider_contracts::submit_job!(&FeedbackFactsJob);
