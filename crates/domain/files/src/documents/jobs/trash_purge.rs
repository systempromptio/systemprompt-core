//! The document trash purge job: hard-deletes documents trashed longer than
//! `files.documents.trashRetentionDays`. Their revisions, node ids, grants
//! and comments go with them; the blob GC then collects the blobs no
//! revision references any more.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::repository::DocumentRepository;

const BATCH: i64 = 200;

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentTrashPurgeJob;

impl DocumentTrashPurgeJob {
    pub async fn purge(db: &DbPool, retention_days: u32) -> ProviderResult<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(retention_days));
        let documents = DocumentRepository::new(db);
        let mut purged = 0;
        loop {
            let batch = documents
                .purge_trashed_before(cutoff, BATCH)
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
impl Job for DocumentTrashPurgeJob {
    fn name(&self) -> &'static str {
        "document_trash_purge"
    }

    fn description(&self) -> &'static str {
        "Deletes documents trashed longer than the retention period"
    }

    fn schedule(&self) -> &'static str {
        "0 45 2 * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let documents = *ctx.get::<Arc<FilesConfig>>()?.documents_config();
        if !documents.is_enabled() {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let started = std::time::Instant::now();
        let purged = Self::purge(ctx.get::<DbPool>()?, documents.trash_retention_days).await?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(purged, duration_ms, "Document trash purge completed");
        Ok(JobResult::success()
            .with_stats(purged, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentTrashPurgeJob);
