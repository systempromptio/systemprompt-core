//! The collab reaper job: settles and closes edit sessions no replica is
//! driving any more.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::service::DocumentService;

const BATCH: i64 = 50;
const IDLE: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentCollabReaperJob;

impl DocumentCollabReaperJob {
    async fn run(ctx: &JobContext) -> ProviderResult<JobResult> {
        if !ctx
            .get::<Arc<FilesConfig>>()?
            .documents_config()
            .is_enabled()
        {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let Some(collab) = ctx.get::<Arc<DocumentService>>()?.collab() else {
            return Ok(JobResult::success().with_message("live co-editing not attached"));
        };
        let started = std::time::Instant::now();
        let reaped = collab
            .reap_idle(IDLE, BATCH)
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::debug!(
            closed = reaped.closed,
            left_open = reaped.left_open,
            failed = reaped.failed,
            duration_ms,
            "Document collab reaper completed"
        );
        Ok(JobResult::success()
            .with_stats(reaped.closed, reaped.failed)
            .with_duration(duration_ms))
    }
}

#[async_trait]
impl Job for DocumentCollabReaperJob {
    fn name(&self) -> &'static str {
        "document_collab_reaper"
    }

    fn description(&self) -> &'static str {
        "Settles and closes document edit sessions left idle with no editor present"
    }

    fn schedule(&self) -> &'static str {
        "0 * * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        Box::pin(act_as(DocumentActor::system(self.name()), Self::run(ctx))).await
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentCollabReaperJob);
