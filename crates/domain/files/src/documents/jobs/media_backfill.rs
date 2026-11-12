//! The document media backfill job: admits the images named by revisions
//! written before admissions were recorded, for every document still in the
//! `document_media_backfill` queue, in bounded batches until the queue is
//! empty. Blob garbage collection holds every blob while the queue is not.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::service::DocumentService;

const BATCH: i64 = 50;

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentMediaBackfillJob;

impl DocumentMediaBackfillJob {
    async fn run(ctx: &JobContext) -> ProviderResult<JobResult> {
        let documents = ctx.get::<Arc<FilesConfig>>()?.documents_config().clone();
        if !documents.is_enabled() {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let service = Arc::clone(ctx.get::<Arc<DocumentService>>()?);
        let started = std::time::Instant::now();
        let mut backfilled = 0;
        loop {
            let done = service
                .backfill_queued_media(BATCH)
                .await
                .map_err(|error| ProviderError::Internal(Box::new(error)))?;
            backfilled += done;
            if done < BATCH.unsigned_abs() {
                break;
            }
        }
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if backfilled > 0 {
            tracing::info!(backfilled, duration_ms, "Document media backfill completed");
        }
        Ok(JobResult::success()
            .with_stats(backfilled, 0)
            .with_duration(duration_ms))
    }
}

#[async_trait]
impl Job for DocumentMediaBackfillJob {
    fn name(&self) -> &'static str {
        "document_media_backfill"
    }

    fn description(&self) -> &'static str {
        "Admits the images named by document revisions written before admissions were recorded"
    }

    fn schedule(&self) -> &'static str {
        "0 */10 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        act_as(DocumentActor::system(self.name()), Self::run(ctx)).await
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentMediaBackfillJob);
