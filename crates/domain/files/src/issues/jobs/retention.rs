//! The issue report retention job.
//!
//! It deletes reports resolved or closed longer than
//! `files.issues.retentionDays`, in bounded batches. Their attachment
//! rows cascade, and the blob GC then collects the blobs nothing references.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::issues::repository::IssueReportRepository;

const BATCH: i64 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct IssueRetentionJob;

impl IssueRetentionJob {
    pub async fn purge(db: &DbPool, retention_days: u32) -> ProviderResult<u64> {
        let reports = IssueReportRepository::new(db);
        let mut purged = 0;
        loop {
            let batch = reports
                .purge_closed(retention_days, BATCH)
                .await
                .map_err(|error| ProviderError::Internal(Box::new(error)))?;
            purged += batch;
            if batch < BATCH.unsigned_abs() {
                return Ok(purged);
            }
        }
    }
}

#[async_trait]
impl Job for IssueRetentionJob {
    fn name(&self) -> &'static str {
        "issue_report_retention"
    }

    fn description(&self) -> &'static str {
        "Deletes issue reports resolved or closed longer than the retention period"
    }

    fn schedule(&self) -> &'static str {
        "0 55 2 * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let issues = ctx.get::<Arc<FilesConfig>>()?.issues_config().clone();
        if !issues.is_enabled() {
            return Ok(JobResult::success().with_message("issue reports disabled"));
        }
        let started = std::time::Instant::now();
        let purged = Self::purge(ctx.get::<DbPool>()?, issues.retention_days).await?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(purged, duration_ms, "Issue report retention completed");
        Ok(JobResult::success()
            .with_stats(purged, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&IssueRetentionJob);
