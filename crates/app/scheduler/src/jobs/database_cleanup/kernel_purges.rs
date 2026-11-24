//! The retention purges of the kernel's own tables — `logs` and
//! `analytics_events` (logging), `event_outbox` (events) and
//! `governance_decisions` (governance) — contributed by the scheduler member
//! the same way every capability contributes its tables' purges — and the
//! bound on the scheduler's own `scheduled_job_runs`: a sweep that, under
//! `enforce`, deletes run rows older than [`JOB_RUN_RETENTION_DAYS`] in
//! batches, keeping each job's latest instant for the missed-run policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::DurableOutbox;
use systemprompt_extension::{Contributions, RetentionDecl};
use systemprompt_governance::GovernanceDecisionRetention;
use systemprompt_identifiers::InstanceId;
use systemprompt_logging::{AnalyticsRepository, LoggingRepository};
use systemprompt_traits::{
    JobContext, ProviderError, ProviderResult, RetentionPurge, RetentionSweep,
};

use crate::repository::JobRunRepository;

const JOB_RUN_RETENTION_DAYS: i64 = 7;
const JOB_RUN_BATCH: i64 = 5000;
const JOB_RUN_MAX_BATCHES: usize = 200;

fn internal<E: std::error::Error + Send + Sync + 'static>(error: E) -> ProviderError {
    ProviderError::Internal(Box::new(error))
}

struct LogsPurge;

#[async_trait]
impl RetentionPurge for LogsPurge {
    fn table(&self) -> &'static str {
        "logs"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        LoggingRepository::new(&db)
            .purge_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct AnalyticsEventsPurge;

#[async_trait]
impl RetentionPurge for AnalyticsEventsPurge {
    fn table(&self) -> &'static str {
        "analytics_events"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        AnalyticsRepository::new(&db)
            .purge_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct EventOutboxPurge;

#[async_trait]
impl RetentionPurge for EventOutboxPurge {
    fn table(&self) -> &'static str {
        "event_outbox"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        let instance_id = ctx.get::<InstanceId>()?.clone();
        DurableOutbox::new(db.write_pool().as_ref().clone(), instance_id)
            .purge_processed_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct GovernanceDecisionsPurge;

#[async_trait]
impl RetentionPurge for GovernanceDecisionsPurge {
    fn table(&self) -> &'static str {
        "governance_decisions"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        GovernanceDecisionRetention::new(&db)
            .purge_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct JobRunsSweep;

#[async_trait]
impl RetentionSweep for JobRunsSweep {
    fn name(&self) -> &'static str {
        "scheduled_job_runs"
    }

    async fn sweep(&self, ctx: &JobContext, enforce: bool) -> ProviderResult<()> {
        if !enforce {
            return Ok(());
        }
        let db = Arc::clone(ctx.get::<DbPool>()?);
        let runs = JobRunRepository::new(&db);
        let cutoff = Utc::now() - chrono::Duration::days(JOB_RUN_RETENTION_DAYS);
        for _ in 0..JOB_RUN_MAX_BATCHES {
            let deleted = runs
                .purge_settled_before(cutoff, JOB_RUN_BATCH)
                .await
                .map_err(internal)?;
            if deleted < u64::try_from(JOB_RUN_BATCH).unwrap_or(u64::MAX) {
                break;
            }
        }
        Ok(())
    }
}

pub(crate) fn contribute(c: &mut Contributions<'_>) {
    c.retention(RetentionDecl::Purge(Arc::new(LogsPurge)))
        .retention(RetentionDecl::Purge(Arc::new(AnalyticsEventsPurge)))
        .retention(RetentionDecl::Purge(Arc::new(EventOutboxPurge)))
        .retention(RetentionDecl::Purge(Arc::new(GovernanceDecisionsPurge)))
        .retention(RetentionDecl::Sweep(Arc::new(JobRunsSweep)));
}
