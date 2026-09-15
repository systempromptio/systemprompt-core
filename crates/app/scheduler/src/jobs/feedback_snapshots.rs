//! Configured-owner aggregate processing and bounded custom-range jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::SchedulerError;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::AnalyticsWorkerId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderResult};

#[derive(Debug, Clone, Copy)]
/// Processes snapshots and range operations under the explicitly configured
/// owner.
pub struct FeedbackSnapshotsJob;
#[async_trait]
impl Job for FeedbackSnapshotsJob {
    fn name(&self) -> &'static str {
        "feedback_snapshot_processing"
    }
    fn description(&self) -> &'static str {
        "Refreshes durable feedback snapshots from fenced evidence deltas"
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
        let owner = &ctx.actor().user_id;
        let repository = app.feedback_snapshots_repository();
        let worker = AnalyticsWorkerId::generate();
        let now = chrono::Utc::now();
        let mut processed = 0u64;
        for _ in 0..16 {
            let count = repository
                .process(owner, &worker, now)
                .await
                .map_err(SchedulerError::from)?;
            processed += count;
            if count == 0 {
                break;
            }
        }
        let mut resources = Vec::new();
        let mut after = None;
        for _ in 0..100 {
            let entries = app
                .managed_repository()
                .inventory(owner, after.as_ref(), 100)
                .await
                .map_err(|error| SchedulerError::config_error(error.to_string()))?;
            let len = entries.len();
            after = entries.last().map(|entry| entry.entry_id.clone());
            resources.extend(entries.into_iter().filter_map(|entry| entry.resource_id));
            if len < 100 {
                break;
            }
        }
        app.managed_repository()
            .refresh_installation_coverage(owner)
            .await
            .map_err(|error| SchedulerError::config_error(error.to_string()))?;
        repository
            .refresh(owner, &resources, now)
            .await
            .map_err(SchedulerError::from)?;
        for _ in 0..8 {
            let Some(lease) = repository
                .claim_range(owner, &worker)
                .await
                .map_err(SchedulerError::from)?
            else {
                break;
            };
            repository
                .complete_range(owner, &lease, now)
                .await
                .map_err(SchedulerError::from)?;
        }
        Ok(JobResult::success().with_stats(processed, 0))
    }
}
systemprompt_provider_contracts::submit_job!(&FeedbackSnapshotsJob);
