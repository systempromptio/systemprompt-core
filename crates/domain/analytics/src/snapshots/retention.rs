//! Retention erases evidence only behind committed producer, fact and snapshot
//! barriers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackSnapshotsRepository, RetentionOutcome, invalid};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

impl FeedbackSnapshotsRepository {
    pub async fn compact_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        now: DateTime<Utc>,
    ) -> crate::Result<RetentionOutcome> {
        if now > Utc::now() {
            return Err(invalid("Retention clock cannot be in the future"));
        }
        Self::lock_retention_barriers(tx, owner).await?;
        let cutoff = now - chrono::Duration::days(90);
        let day = cutoff.date_naive();
        let oldest = now.date_naive() - chrono::Duration::days(364);
        let previous = sqlx::query_scalar!(
            "SELECT evidence_cutoff FROM analytics_snapshot_state WHERE owner_id=$1",
            owner.as_str()
        )
        .fetch_one(&mut **tx)
        .await?;
        if previous.is_some_and(|previous| cutoff < previous) {
            return Err(invalid("Retention cutoff cannot move backwards"));
        }
        let outcome = Self::purge_expired(tx, owner, cutoff, oldest).await?;
        let days: Vec<_> = (0..90)
            .map(|offset| now.date_naive() - chrono::Duration::days(offset))
            .collect();
        let generation = sqlx::query_scalar!(
            "SELECT fact_generation FROM analytics_snapshot_state WHERE owner_id=$1",
            owner.as_str()
        )
        .fetch_one(&mut **tx)
        .await?;
        Self::rebuild_days(tx, owner, &days, generation).await?;
        Self::refresh_in(tx, owner, &[], now).await?;
        sqlx::query!(
            "SELECT public.finish_reporting_compaction($1) AS processed",
            now - chrono::Duration::days(90)
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(RetentionOutcome {
            compacted_before: day,
            ..outcome
        })
    }
    async fn purge_expired(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        cutoff: DateTime<Utc>,
        oldest: chrono::NaiveDate,
    ) -> crate::Result<RetentionOutcome> {
        let day = cutoff.date_naive();
        sqlx::query!("INSERT INTO analytics_snapshot_dirty(owner_id,scope) SELECT owner_id,scope FROM analytics_feedback_snapshots WHERE owner_id=$1 ON CONFLICT DO NOTHING",owner.as_str()).execute(&mut **tx).await?;
        sqlx::query!("UPDATE analytics_snapshot_daily SET metrics='{}'::jsonb,spend='{}'::jsonb,histogram='{}'::jsonb,cohort=0,suppressed=true WHERE owner_id=$1 AND ((day<$2 AND cohort<5) OR day=$2)",owner.as_str(),day).execute(&mut **tx).await?;
        sqlx::query!(
            "DELETE FROM analytics_snapshot_identities WHERE owner_id=$1 AND day<=$2",
            owner.as_str(),
            day
        )
        .execute(&mut **tx)
        .await?;
        let removed = Self::erase_expired(tx, owner, cutoff).await?;
        sqlx::query!(
            "DELETE FROM analytics_fact_changes WHERE owner_id=$1 AND occurred_at<$2",
            owner.as_str(),
            cutoff
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!("DELETE FROM analytics_fact_deltas WHERE owner_id=$1 AND (occurred_at<$2 OR (before_fact->'value'->>'occurred_at')::timestamptz<$2 OR (after_fact->'value'->>'occurred_at')::timestamptz<$2)",owner.as_str(),cutoff).execute(&mut **tx).await?;
        sqlx::query!(
            "DELETE FROM analytics_fact_backfill_pages WHERE owner_id=$1",
            owner.as_str()
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM analytics_fact_backfills WHERE owner_id=$1",
            owner.as_str()
        )
        .execute(&mut **tx)
        .await?;
        let removed_daily = sqlx::query!(
            "DELETE FROM analytics_snapshot_daily WHERE owner_id=$1 AND day<$2",
            owner.as_str(),
            oldest
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        sqlx::query!(
            "DELETE FROM analytics_snapshot_jobs WHERE owner_id=$1 AND created_at<$2",
            owner.as_str(),
            cutoff
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!("UPDATE analytics_snapshot_jobs SET state='pending',result=NULL,lease_worker=NULL,lease_until=NULL,lease_epoch=lease_epoch+1,last_error='Range invalidated by privacy retention' WHERE owner_id=$1",owner.as_str()).execute(&mut **tx).await?;
        sqlx::query!("UPDATE analytics_snapshot_state SET compacted_before=$2,evidence_cutoff=$3 WHERE owner_id=$1",owner.as_str(),day,cutoff).execute(&mut **tx).await?;
        Ok(RetentionOutcome {
            compacted_before: day,
            removed_facts: removed,
            removed_daily,
        })
    }
    async fn lock_retention_barriers(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
    ) -> crate::Result<()> {
        sqlx::query!("SELECT public.prepare_reporting_privacy() AS locked")
            .fetch_one(&mut **tx)
            .await?;
        sqlx::query!("LOCK TABLE analytics_ingestion_producers,analytics_fact_checkpoints,analytics_fact_backfills,analytics_fact_changes,analytics_fact_consumers,analytics_fact_deltas IN EXCLUSIVE MODE").execute(&mut **tx).await?;
        let producers=sqlx::query!("SELECT producer,pending_count FROM analytics_ingestion_producers ORDER BY producer FOR UPDATE").fetch_all(&mut **tx).await?;
        if producers.iter().any(|row| row.pending_count != 0) {
            return Err(invalid(
                "Retention waits for pending producer evidence and privacy corrections",
            ));
        }
        let facts = sqlx::query_scalar!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| invalid("Fact checkpoint is not initialized"))?;
        let state = sqlx::query!(
            "SELECT fact_generation FROM analytics_snapshot_state WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| invalid("Snapshots are not initialized"))?;
        let consumers=sqlx::query!("SELECT consumer,generation FROM analytics_fact_consumers WHERE owner_id=$1 ORDER BY consumer FOR UPDATE",owner.as_str()).fetch_all(&mut **tx).await?;
        let pending=sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM analytics_fact_changes WHERE owner_id=$1 AND state IN('pending','leased')) OR EXISTS(SELECT 1 FROM analytics_fact_deltas WHERE owner_id=$1 AND consumed_at IS NULL) OR EXISTS(SELECT 1 FROM analytics_fact_backfills WHERE owner_id=$1 AND NOT complete) AS "pending!""#,owner.as_str()).fetch_one(&mut **tx).await?;
        if pending
            || state.fact_generation != facts
            || !consumers.iter().any(|row| row.consumer == "snapshots-v1")
            || consumers.iter().any(|row| row.generation != facts)
        {
            return Err(invalid(
                "Retention waits for committed fact and aggregate checkpoints",
            ));
        }
        Ok(())
    }
}
