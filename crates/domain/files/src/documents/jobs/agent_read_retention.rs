//! The agent read retention job: deletes `document_agent_reads` rows older
//! than `files.documents.agentReadRetentionDays`, a bounded batch at a time,
//! until the window is clear or the run's time budget is spent. It runs
//! hourly, so a budget spent on a backlog resumes from the oldest row within
//! the hour rather than the next day.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::repository::DocumentRepository;

const BATCH: i64 = 5000;
const TIME_BUDGET: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentAgentReadRetentionJob;

impl DocumentAgentReadRetentionJob {
    pub async fn purge(db: &DbPool, retention_days: u32) -> ProviderResult<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(retention_days));
        let documents = DocumentRepository::new(db);
        let started = std::time::Instant::now();
        let mut purged = 0;
        loop {
            let batch = documents
                .purge_agent_reads_before(cutoff, BATCH)
                .await
                .map_err(|error| ProviderError::Internal(Box::new(error)))?;
            purged += batch;
            if batch < BATCH.unsigned_abs() {
                return Ok(purged);
            }
            if started.elapsed() >= TIME_BUDGET {
                tracing::warn!(
                    purged,
                    "Document agent read retention stopped at its time budget with rows still \
                     past the window"
                );
                return Ok(purged);
            }
        }
    }

    async fn run(ctx: &JobContext) -> ProviderResult<JobResult> {
        let documents = *ctx.get::<Arc<FilesConfig>>()?.documents_config();
        if !documents.is_enabled() {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let started = std::time::Instant::now();
        let purged = Self::purge(ctx.get::<DbPool>()?, documents.agent_read_retention_days).await?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            purged,
            duration_ms,
            "Document agent read retention completed"
        );
        Ok(JobResult::success()
            .with_stats(purged, 0)
            .with_duration(duration_ms))
    }
}

#[async_trait]
impl Job for DocumentAgentReadRetentionJob {
    fn name(&self) -> &'static str {
        "document_agent_read_retention"
    }

    fn description(&self) -> &'static str {
        "Deletes agent read audit rows older than the retention period"
    }

    fn schedule(&self) -> &'static str {
        "0 15 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        act_as(DocumentActor::system(self.name()), Self::run(ctx)).await
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentAgentReadRetentionJob);
