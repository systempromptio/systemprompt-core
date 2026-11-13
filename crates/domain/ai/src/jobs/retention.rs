//! The AI tables `database_cleanup` prunes — stored request messages, raw
//! request payload bodies and spent quota buckets — and the orphan sweep it
//! runs first: requests left `pending` by a lost settlement are failed with
//! unknown usage. `journal::recover()` runs that settlement only at boot, so
//! a long-lived server relies on this sweep as its regular caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_traits::{
    JobContext, ProviderError, ProviderResult, RetentionPurge, RetentionSweep,
    register_retention_purge, register_retention_sweep,
};
use tracing::{info, warn};

use crate::repository::ai_requests::ORPHAN_AGE;
use crate::repository::{AiQuotaBucketRepository, AiRequestPayloadRepository, AiRequestRepository};

fn internal<E: std::error::Error + Send + Sync + 'static>(error: E) -> ProviderError {
    ProviderError::Internal(Box::new(error))
}

struct RequestMessagesPurge;

#[async_trait]
impl RetentionPurge for RequestMessagesPurge {
    fn table(&self) -> &'static str {
        "ai_request_messages"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        AiRequestRepository::new(&db)
            .purge_messages_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct RequestPayloadsPurge;

#[async_trait]
impl RetentionPurge for RequestPayloadsPurge {
    fn table(&self) -> &'static str {
        "ai_request_payloads"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        AiRequestPayloadRepository::new(&db)
            .release_bodies_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct QuotaBucketsPurge;

#[async_trait]
impl RetentionPurge for QuotaBucketsPurge {
    fn table(&self) -> &'static str {
        "ai_quota_buckets"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        AiQuotaBucketRepository::new(&db)
            .purge_before(cutoff, batch)
            .await
            .map_err(internal)
    }
}

struct OrphanedRequestsSweep;

#[async_trait]
impl RetentionSweep for OrphanedRequestsSweep {
    fn name(&self) -> &'static str {
        "ai_orphaned_requests"
    }

    async fn sweep(&self, ctx: &JobContext, enforce: bool) -> ProviderResult<()> {
        if !enforce {
            info!("enforce disabled: orphaned pending AI requests were not failed");
            return Ok(());
        }
        let db = Arc::clone(ctx.get::<DbPool>()?);
        let orphans = AiRequestRepository::new(&db)
            .fail_orphaned_pending(ORPHAN_AGE)
            .await
            .map_err(internal)?;
        for orphan in &orphans {
            warn!(
                ai_request_id = %orphan.id,
                user_id = %orphan.owner,
                "AI request outlived every receipt; failed with unknown usage"
            );
        }
        Ok(())
    }
}

register_retention_purge!(&RequestMessagesPurge);
register_retention_purge!(&RequestPayloadsPurge);
register_retention_purge!(&QuotaBucketsPurge);
register_retention_sweep!(&OrphanedRequestsSweep);
