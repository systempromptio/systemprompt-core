//! The retention purges of the kernel's own tables — `logs` and
//! `analytics_events` (logging), `event_outbox` (events) and
//! `governance_decisions` (governance) — registered through the same
//! [`RetentionPurge`] registry every capability declares its tables in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::DurableOutbox;
use systemprompt_governance::GovernanceDecisionRetention;
use systemprompt_logging::{AnalyticsRepository, LoggingRepository};
use systemprompt_traits::{
    JobContext, ProviderError, ProviderResult, RetentionPurge, register_retention_purge,
};

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

register_retention_purge!("scheduler", &LogsPurge);
register_retention_purge!("scheduler", &AnalyticsEventsPurge);
register_retention_purge!("scheduler", &EventOutboxPurge);
register_retention_purge!("scheduler", &GovernanceDecisionsPurge);
