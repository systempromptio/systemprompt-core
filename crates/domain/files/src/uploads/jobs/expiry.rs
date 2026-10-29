//! The upload expiry job: closes upload sessions left open past their
//! lifetime, deletes the parts they stored, and deletes a bounded batch of
//! orphaned upload objects. An object whose delete fails stays recorded
//! and is retried on the next run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionContext;
use systemprompt_traits::{
    BackgroundTasks, Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult,
};

use super::super::repository::DocumentUploadRepository;
use super::super::service::{UploadService, UploadServiceParts};
use crate::FilesConfig;
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::blob_store::BlobStore;
use crate::documents::repository::DocumentBlobRepository;

const BATCH: i64 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentUploadExpiryJob;

impl DocumentUploadExpiryJob {
    async fn run(ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = std::time::Instant::now();
        let db_pool = ctx.get::<DbPool>()?;
        let documents = *ctx.get::<Arc<FilesConfig>>()?.documents_config();
        if !documents.is_enabled() {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let storage = ctx.get::<Arc<dyn ExtensionContext>>()?.file_storage();
        let blobs = Arc::new(BlobStore::new(
            Arc::clone(&storage),
            Arc::new(DocumentBlobRepository::new(db_pool)),
        ));
        let service = UploadService::new(UploadServiceParts {
            storage,
            uploads: Arc::new(DocumentUploadRepository::new(db_pool)),
            blobs,
            max_upload_bytes: documents.max_upload_bytes,
            limits: documents.uploads,
            tasks: BackgroundTasks::new(),
        });
        let expiry = service
            .expire(BATCH)
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let sweep = service
            .sweep_orphans(BATCH)
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let retained = sweep.retained;
        tracing::info!(
            expired = expiry.expired,
            swept = sweep.deleted,
            retained,
            duration_ms,
            "Document upload expiry completed"
        );
        Ok(JobResult::success()
            .with_stats(expiry.expired + sweep.deleted, retained)
            .with_message(format!(
                "Expired: {}, orphans deleted: {}, orphans retained: {retained}",
                expiry.expired, sweep.deleted
            ))
            .with_duration(duration_ms))
    }
}

#[async_trait]
impl Job for DocumentUploadExpiryJob {
    fn name(&self) -> &'static str {
        "document_upload_expiry"
    }

    fn description(&self) -> &'static str {
        "Closes resumable uploads left open past their lifetime, deletes their parts and sweeps orphaned upload objects"
    }

    fn schedule(&self) -> &'static str {
        "0 */15 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        act_as(DocumentActor::system(self.name()), Self::run(ctx)).await
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentUploadExpiryJob);
