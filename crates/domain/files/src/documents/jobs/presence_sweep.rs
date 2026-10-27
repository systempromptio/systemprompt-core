//! The document presence sweep job: deletes expired presence rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::repository::DocumentPresenceRepository;

const BATCH: i64 = 1000;

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentPresenceSweepJob;

#[async_trait]
impl Job for DocumentPresenceSweepJob {
    fn name(&self) -> &'static str {
        "document_presence_sweep"
    }

    fn description(&self) -> &'static str {
        "Removes expired document presence rows"
    }

    fn schedule(&self) -> &'static str {
        "0 * * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        if !ctx
            .get::<Arc<FilesConfig>>()?
            .documents_config()
            .is_enabled()
        {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let started = std::time::Instant::now();
        let presence = DocumentPresenceRepository::new(ctx.get::<DbPool>()?);
        let swept = presence
            .sweep_expired(BATCH)
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::debug!(swept, duration_ms, "Document presence sweep completed");
        Ok(JobResult::success()
            .with_stats(swept, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentPresenceSweepJob);
