//! The document node id backfill job: mints stable node ids for revisions
//! written before ids existed, so no read path ever writes.
//!
//! It runs once per release across the cluster through
//! [`ReleaseStampRepository::run_once`]: replicas that start together
//! serialise on the stamp, and a release that already ran it skips. Each
//! run pages the revisions with no ids oldest first, so a parent is minted
//! before its child and the child carries its parent's ids. A revision
//! whose bytes cannot be projected is logged and counted as failed; the run
//! still records the release.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::{
    DbPool, ReleaseGate, ReleaseRun, ReleaseStampRepository, StampOutcome,
};
use systemprompt_identifiers::JobName;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult};

use crate::FilesConfig;
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::error::DocumentsError;
use crate::documents::service::DocumentService;

const BATCH: i64 = 100;
const LOCK_WAIT: Duration = Duration::from_secs(5);

/// What one backfill pass did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackfillReport {
    pub minted: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentNodeIdsBackfillJob;

impl DocumentNodeIdsBackfillJob {
    pub async fn backfill(service: &DocumentService) -> Result<BackfillReport, DocumentsError> {
        let ids = &service.repositories().node_ids;
        let revisions = &service.repositories().revisions;
        let mut report = BackfillReport::default();
        let mut after = None;
        loop {
            let page = ids
                .without_ids(after.as_ref().map(|(at, id)| (*at, id)), BATCH)
                .await?;
            let full = i64::try_from(page.len()).unwrap_or(i64::MAX) == BATCH;
            for (at, id) in page {
                if let Some(revision) = revisions.find(&id).await? {
                    match service.backfill_node_ids(&revision).await {
                        Ok(_) => report.minted += 1,
                        Err(error) => {
                            tracing::warn!(revision = %id, error = %error, "document node ids were not backfilled");
                            report.failed += 1;
                        },
                    }
                }
                after = Some((at, id));
            }
            if !full {
                return Ok(report);
            }
        }
    }
}

impl DocumentNodeIdsBackfillJob {
    /// The job's work, run as its named system actor by `execute`.
    async fn run(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let documents = *ctx.get::<Arc<FilesConfig>>()?.documents_config();
        if !documents.is_enabled() {
            return Ok(JobResult::success().with_message("document store disabled"));
        }
        let service = Arc::clone(ctx.get::<Arc<DocumentService>>()?);
        let stamps = ReleaseStampRepository::new(ctx.get::<DbPool>()?);
        let name = JobName::new(self.name());
        let run = ReleaseRun {
            name: &name,
            release: env!("CARGO_PKG_VERSION"),
            gate: ReleaseGate::IfBehind,
            wait: LOCK_WAIT,
        };
        let started = std::time::Instant::now();
        let outcome = stamps
            .run_once(run, || async move { Self::backfill(&service).await })
            .await
            .map_err(|error| ProviderError::Internal(Box::new(error)))?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Ok(match outcome {
            StampOutcome::Ran(report) => {
                tracing::info!(
                    minted = report.minted,
                    failed = report.failed,
                    duration_ms,
                    "Document node id backfill completed"
                );
                JobResult::success()
                    .with_stats(report.minted, report.failed)
                    .with_duration(duration_ms)
            },
            StampOutcome::Current
            | StampOutcome::NewerRecorded { .. }
            | StampOutcome::Contended => {
                JobResult::success().with_message("node ids already backfilled for this release")
            },
        })
    }
}

#[async_trait]
impl Job for DocumentNodeIdsBackfillJob {
    fn name(&self) -> &'static str {
        "document_node_ids_backfill"
    }

    fn description(&self) -> &'static str {
        "Mints stable node ids for document revisions written before ids existed, once per release"
    }

    fn schedule(&self) -> &'static str {
        "0 */10 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        act_as(DocumentActor::system(self.name()), self.run(ctx)).await
    }
}

systemprompt_provider_contracts::submit_job!(&DocumentNodeIdsBackfillJob);
