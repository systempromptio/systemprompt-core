use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionContext;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::blob_store::BlobStore;
use crate::documents::repository::DocumentBlobRepository;

const BATCH: u32 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentBlobGcJob;

#[async_trait]
impl Job for DocumentBlobGcJob {
    fn name(&self) -> &'static str {
        "document_blob_gc"
    }

    fn description(&self) -> &'static str {
        "Deletes document blobs no revision or edit session references"
    }

    fn schedule(&self) -> &'static str {
        "0 15 3 * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = std::time::Instant::now();
        let db_pool = ctx.get::<DbPool>()?;
        let grace_hours = ctx
            .get::<Arc<FilesConfig>>()?
            .documents_config()
            .blob_gc_grace_hours;
        let storage = ctx.get::<Arc<dyn ExtensionContext>>()?.file_storage();
        let store = BlobStore::new(storage, Arc::new(DocumentBlobRepository::new(db_pool)));

        let report = store
            .delete_unreferenced(grace_hours, BATCH)
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        tracing::info!(
            examined = report.examined,
            deleted = report.deleted,
            failed = report.failed,
            duration_ms,
            "Document blob GC completed"
        );

        Ok(JobResult::success()
            .with_stats(report.deleted, report.failed)
            .with_message(format!(
                "Examined: {}, Deleted: {}, Failed: {}",
                report.examined, report.deleted, report.failed
            ))
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentBlobGcJob);
