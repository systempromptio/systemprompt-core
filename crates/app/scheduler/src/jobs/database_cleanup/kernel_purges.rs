//! The retention purges of the kernel's own tables — `logs` and
//! `analytics_events` (logging), `event_outbox` (events) and
//! `governance_decisions` (governance) — contributed by the scheduler member
//! the same way every capability contributes its tables' purges.
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
use systemprompt_logging::{AnalyticsRepository, LoggingRepository};
use systemprompt_traits::{
JobContext, ProviderError, ProviderResult, RetentionPurge};

use crate::services::scheduling::job_app_context;

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
        let instance_id = job_app_context(ctx)?.config().instance_id.clone();
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

pub(crate) fn contribute(c: &mut Contributions<'_>) {
    c.retention(RetentionDecl::Purge(Arc::new(LogsPurge)))
        .retention(RetentionDecl::Purge(Arc::new(AnalyticsEventsPurge)))
        .retention(RetentionDecl::Purge(Arc::new(EventOutboxPurge)))
        .retention(RetentionDecl::Purge(Arc::new(GovernanceDecisionsPurge)));
}
