//! Global raw expiry requires atomic compaction of every initialized
//! organizational scope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackSnapshotsRepository, RetentionSummary, invalid};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

impl FeedbackSnapshotsRepository {
    pub async fn compact_all_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        now: DateTime<Utc>,
    ) -> crate::Result<RetentionSummary> {
        if now > Utc::now() {
            return Err(invalid("Retention clock cannot be in the future"));
        }
        sqlx::query!("SELECT public.prepare_reporting_privacy() AS locked")
            .fetch_one(&mut **tx)
            .await?;
        sqlx::query!("LOCK TABLE analytics_ingestion_producers,analytics_fact_checkpoints,analytics_fact_backfills,analytics_fact_changes,analytics_fact_consumers,analytics_fact_deltas IN EXCLUSIVE MODE").execute(&mut **tx).await?;
        let owners=sqlx::query_scalar!("SELECT owner_id FROM analytics_fact_checkpoints ORDER BY owner_id LIMIT 10001 FOR UPDATE").fetch_all(&mut **tx).await?;
        if owners.len() > 10000 {
            return Err(invalid("Retention organization bound exceeded"));
        }
        let mut summary = RetentionSummary {
            organizations: 0,
            removed_facts: 0,
            removed_daily: 0,
        };
        for owner in owners {
            let result = Self::compact_in(tx, &UserId::new(owner), now).await?;
            summary.organizations += 1;
            summary.removed_facts = summary
                .removed_facts
                .checked_add(result.removed_facts)
                .ok_or_else(|| invalid("Retention count overflow"))?;
            summary.removed_daily = summary
                .removed_daily
                .checked_add(result.removed_daily)
                .ok_or_else(|| invalid("Retention count overflow"))?;
        }
        sqlx::query!(
            "SELECT public.finish_reporting_privacy($1) AS processed",
            now - chrono::Duration::days(90)
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(summary)
    }
}
