//! Retention deletes for the analytics tables that hold personal data.
//!
//! [`AnalyticsRetention`] bounds `engagement_events` by `created_at` and
//! `fingerprint_reputation` by `last_seen_at`, the latter carrying the last
//! IP address and user agent seen for a fingerprint. An idle unflagged
//! fingerprint is deleted; an idle flagged one is kept as enforcement memory
//! with its IP address and user agent set to NULL. Each statement touches at
//! most `batch` rows so the scheduler's retention job can drive it in a
//! bounded loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;

use crate::Result;

/// Batched retention deletes over the analytics tables holding personal data.
#[derive(Clone, Debug)]
pub struct AnalyticsRetention {
    write_pool: Arc<PgPool>,
}

impl AnalyticsRetention {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    pub async fn purge_engagement_events_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM engagement_events WHERE ctid = ANY(ARRAY(SELECT ctid FROM \
             engagement_events WHERE created_at < $1 LIMIT $2))",
            cutoff,
            batch
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_fingerprints_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64> {
        let deleted = sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = ANY(ARRAY(SELECT \
             fingerprint_hash FROM fingerprint_reputation WHERE last_seen_at < $1 AND NOT \
             is_flagged LIMIT $2))",
            cutoff,
            batch
        )
        .execute(&*self.write_pool)
        .await?
        .rows_affected();
        let scrubbed = sqlx::query!(
            "UPDATE fingerprint_reputation SET last_ip_address = NULL, last_user_agent = NULL \
             WHERE fingerprint_hash = ANY(ARRAY(SELECT fingerprint_hash FROM \
             fingerprint_reputation WHERE last_seen_at < $1 AND is_flagged AND \
             (last_ip_address IS NOT NULL OR last_user_agent IS NOT NULL) LIMIT $2))",
            cutoff,
            batch
        )
        .execute(&*self.write_pool)
        .await?
        .rows_affected();
        Ok(deleted + scrubbed)
    }
}
