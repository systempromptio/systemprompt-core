//! Fenced delta processing updates shadows, daily totals and durable consumer
//! checkpoints atomically.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::daily::FactReference;
use super::{FeedbackSnapshotsRepository, invalid};
use crate::feedback::{DeltaClaim, DeltaLease, FeedbackFactsRepository};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::BTreeSet;
use systemprompt_identifiers::{AnalyticsWorkerId, UserId};

#[derive(Debug, Clone, Copy)]
struct ShadowWindow {
    cutoff: NaiveDate,
    generation: i64,
}

impl FeedbackSnapshotsRepository {
    pub async fn process(
        &self,
        owner: &UserId,
        worker: &AnalyticsWorkerId,
        now: DateTime<Utc>,
    ) -> crate::Result<u64> {
        let Some(lease) = self
            .facts
            .claim_deltas(
                owner,
                "snapshots-v1",
                worker,
                DeltaClaim {
                    limit: 256,
                    lease_seconds: 300,
                },
            )
            .await?
        else {
            return Ok(0);
        };
        let result = self.apply_batch(owner, &lease, now).await;
        if result.is_err() {
            sqlx::query!("INSERT INTO analytics_snapshot_state(owner_id,last_error) VALUES($1,'Snapshot batch failed; leased work remains retryable') ON CONFLICT(owner_id) DO UPDATE SET last_error=EXCLUDED.last_error",owner.as_str()).execute(&self.pool).await?;
        }
        result
    }
    pub async fn apply_batch(
        &self,
        owner: &UserId,
        lease: &DeltaLease,
        now: DateTime<Utc>,
    ) -> crate::Result<u64> {
        if lease.consumer != "snapshots-v1" {
            return Err(invalid("Invalid snapshot consumer"));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO analytics_snapshot_state(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        let state = sqlx::query!(
            "SELECT compacted_before FROM analytics_snapshot_state WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        FeedbackFactsRepository::lock_delta_lease(&mut tx, owner, lease).await?;
        let rows=sqlx::query!(r#"SELECT generation,fact_kind,source,fact_id,before_fact,after_fact,occurred_at,(SELECT revision FROM analytics_normalized_facts f WHERE f.owner_id=d.owner_id AND f.fact_kind=d.fact_kind AND f.source=d.source AND f.fact_id=d.fact_id) AS "revision?" FROM analytics_fact_deltas d WHERE owner_id=$1 AND generation>$2 AND generation<=$3 ORDER BY generation LIMIT 256"#,owner.as_str(),lease.after_generation,lease.through_generation).fetch_all(&mut *tx).await?;
        let count =
            u64::try_from(rows.len()).map_err(|_error| invalid("Snapshot batch overflow"))?;
        let cutoff = state
            .compacted_before
            .unwrap_or_else(|| now.date_naive() - chrono::Duration::days(365));
        let mut days = BTreeSet::new();
        for row in rows {
            if state.compacted_before.is_some()
                && row.before_fact.is_none()
                && row.revision.is_some_and(|revision| revision > 1)
            {
                Self::suppress_sealed_history(&mut tx, owner, cutoff).await?;
            }
            for fact in [&row.before_fact, &row.after_fact] {
                let reference = FactReference {
                    kind: &row.fact_kind,
                    source: &row.source,
                    id: &row.fact_id,
                    fact: fact.as_ref(),
                };
                days.extend(Self::affected_days(&mut tx, owner, &reference).await?);
            }
            days.insert(row.occurred_at.date_naive());
            if row.occurred_at.date_naive() < cutoff {
                Self::suppress_day(&mut tx, owner, row.occurred_at.date_naive(), row.generation)
                    .await?;
            }
            let reference = FactReference {
                kind: &row.fact_kind,
                source: &row.source,
                id: &row.fact_id,
                fact: row.after_fact.as_ref(),
            };
            let shadow = ShadowWindow {
                cutoff,
                generation: row.generation,
            };
            if let Some(day) = Self::replace_shadow(&mut tx, owner, &reference, shadow).await? {
                days.insert(day);
            }
            days.extend(Self::affected_days(&mut tx, owner, &reference).await?);
        }
        Self::finish_batch(&mut tx, owner, lease, &days, cutoff).await?;
        tx.commit().await?;
        Ok(count)
    }

    async fn suppress_sealed_history(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        cutoff: NaiveDate,
    ) -> crate::Result<()> {
        sqlx::query!("INSERT INTO analytics_snapshot_dirty(owner_id,scope) SELECT owner_id,scope FROM analytics_snapshot_daily WHERE owner_id=$1 AND day<$2 ON CONFLICT DO NOTHING",owner.as_str(),cutoff).execute(&mut **tx).await?;
        sqlx::query!("UPDATE analytics_snapshot_daily SET metrics='{}'::jsonb,spend='{}'::jsonb,histogram='{}'::jsonb,cohort=0,suppressed=true WHERE owner_id=$1 AND day<$2",owner.as_str(),cutoff).execute(&mut **tx).await?;
        sqlx::query!("UPDATE analytics_snapshot_jobs SET state='pending',result=NULL,lease_worker=NULL,lease_until=NULL,lease_epoch=lease_epoch+1,last_error='Sealed history suppressed after a correction without retained provenance' WHERE owner_id=$1 AND from_day<$2",owner.as_str(),cutoff).execute(&mut **tx).await?;
        Ok(())
    }

    async fn suppress_day(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        day: NaiveDate,
        generation: i64,
    ) -> crate::Result<()> {
        sqlx::query!("INSERT INTO analytics_snapshot_dirty(owner_id,scope) SELECT owner_id,scope FROM analytics_snapshot_daily WHERE owner_id=$1 AND day=$2 ON CONFLICT DO NOTHING",owner.as_str(),day).execute(&mut **tx).await?;
        sqlx::query!("UPDATE analytics_snapshot_daily SET metrics='{}'::jsonb,spend='{}'::jsonb,histogram='{}'::jsonb,cohort=0,suppressed=true,generation=$3 WHERE owner_id=$1 AND day=$2",owner.as_str(),day,generation).execute(&mut **tx).await?;
        Ok(())
    }

    async fn replace_shadow(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        reference: &FactReference<'_>,
        shadow: ShadowWindow,
    ) -> crate::Result<Option<NaiveDate>> {
        sqlx::query!("DELETE FROM analytics_snapshot_shadow WHERE owner_id=$1 AND fact_kind=$2 AND source=$3 AND fact_id=$4",owner.as_str(),reference.kind,reference.source,reference.id).execute(&mut **tx).await?;
        let Some(fact) = reference.fact else {
            return Ok(None);
        };
        let occurred = fact
            .get("value")
            .and_then(|value| value.get("occurred_at"))
            .cloned()
            .ok_or_else(|| invalid("Missing event timestamp"))?;
        let occurred: DateTime<Utc> = serde_json::from_value(occurred)?;
        if occurred.date_naive() >= shadow.cutoff {
            sqlx::query!("INSERT INTO analytics_snapshot_shadow(owner_id,fact_kind,source,fact_id,fact,occurred_at,generation) VALUES($1,$2,$3,$4,$5,$6,$7)",owner.as_str(),reference.kind,reference.source,reference.id,fact,occurred,shadow.generation).execute(&mut **tx).await?;
        }
        Ok(Some(occurred.date_naive()))
    }

    async fn finish_batch(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        lease: &DeltaLease,
        days: &BTreeSet<NaiveDate>,
        cutoff: NaiveDate,
    ) -> crate::Result<()> {
        let all_days: Vec<_> = days.iter().copied().collect();
        sqlx::query!("UPDATE analytics_snapshot_jobs SET state='pending',result=NULL,lease_worker=NULL,lease_until=NULL,lease_epoch=lease_epoch+1,last_error='Range invalidated by corrected evidence' WHERE owner_id=$1 AND EXISTS(SELECT 1 FROM unnest($2::date[]) d WHERE d>=from_day AND d<to_day)",owner.as_str(),&all_days).execute(&mut **tx).await?;
        let days: Vec<_> = days.iter().copied().filter(|day| *day >= cutoff).collect();
        Self::rebuild_days(tx, owner, &days, lease.through_generation).await?;
        FeedbackFactsRepository::complete_delta_batch(tx, owner, lease).await?;
        sqlx::query!("UPDATE analytics_snapshot_state SET fact_generation=$2,last_error=NULL WHERE owner_id=$1",owner.as_str(),lease.through_generation).execute(&mut **tx).await?;
        Ok(())
    }
}
