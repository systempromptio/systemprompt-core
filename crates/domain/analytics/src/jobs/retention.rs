//! The analytics tables `database_cleanup` prunes: `engagement_events` past
//! `retention.engagement_events_days` and idle `fingerprint_reputation` rows
//! past `retention.fingerprint_reputation_days`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_traits::{
    JobContext, ProviderError, ProviderResult, RetentionPurge, register_retention_purge,
};

use crate::repository::AnalyticsRetention;

struct EngagementEventsPurge;

#[async_trait]
impl RetentionPurge for EngagementEventsPurge {
    fn table(&self) -> &'static str {
        "engagement_events"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        AnalyticsRetention::new(&db)
            .purge_engagement_events_before(cutoff, batch)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))
    }
}

struct FingerprintReputationPurge;

#[async_trait]
impl RetentionPurge for FingerprintReputationPurge {
    fn table(&self) -> &'static str {
        "fingerprint_reputation"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        AnalyticsRetention::new(&db)
            .purge_fingerprints_before(cutoff, batch)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))
    }
}

register_retention_purge!(&EngagementEventsPurge);
register_retention_purge!(&FingerprintReputationPurge);
