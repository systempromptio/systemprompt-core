//! The collab update sweep job: deletes the update log of closed edit sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::repository::DocumentCollabRepository;

const BATCH: i64 = 1000;

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentCollabUpdateSweepJob;

impl DocumentCollabUpdateSweepJob {
    /// The job's work, run as its named system actor by `execute`.
    async fn run(ctx: &JobContext) -> ProviderResult<JobResult> {
        if !ctx
            .get::<Arc<FilesConfig>>()?
            .documents_config()
            .is_enabled()
        {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let started = std::time::Instant::now();
        let collab = DocumentCollabRepository::new(ctx.get::<DbPool>()?);
        let swept = collab
            .sweep_closed(BATCH)
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::debug!(swept, duration_ms, "Document collab update sweep completed");
        Ok(JobResult::success()
            .with_stats(swept, 0)
            .with_duration(duration_ms))
    }
}

#[async_trait]
impl Job for DocumentCollabUpdateSweepJob {
    fn name(&self) -> &'static str {
        "document_collab_update_sweep"
    }

    fn description(&self) -> &'static str {
        "Removes the update log of closed document edit sessions"
    }

    fn schedule(&self) -> &'static str {
        "0 */5 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        act_as(DocumentActor::system(self.name()), Self::run(ctx)).await
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentCollabUpdateSweepJob);
