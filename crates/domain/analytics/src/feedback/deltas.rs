//! Fenced downstream batches support aggregate writes and checkpoint commit in
//! one transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{DeltaClaim, DeltaLease, FactDelta, FeedbackFactsRepository, validation};
use crate::Result;
use systemprompt_identifiers::{AnalyticsWorkerId, UserId};

impl FeedbackFactsRepository {
    pub async fn claim_deltas(
        &self,
        owner: &UserId,
        consumer: &str,
        worker: &AnalyticsWorkerId,
        claim: DeltaClaim,
    ) -> Result<Option<DeltaLease>> {
        let DeltaClaim {
            limit,
            lease_seconds,
        } = claim;
        if consumer.is_empty()
            || consumer.len() > 128
            || consumer.chars().any(char::is_control)
            || !(1..=256).contains(&limit)
            || !(1..=300).contains(&lease_seconds)
        {
            return Err(validation::invalid());
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO analytics_fact_checkpoints(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!("INSERT INTO analytics_fact_consumers(owner_id,consumer) VALUES($1,$2) ON CONFLICT DO NOTHING", owner.as_str(), consumer).execute(&mut *tx).await?;
        let state = sqlx::query!("SELECT generation FROM analytics_fact_consumers WHERE owner_id=$1 AND consumer=$2 AND (lease_until IS NULL OR lease_until<=clock_timestamp()) FOR UPDATE SKIP LOCKED", owner.as_str(), consumer).fetch_optional(&mut *tx).await?;
        let Some(state) = state else {
            tx.commit().await?;
            return Ok(None);
        };
        let limit = i64::from(limit);
        let through = sqlx::query_scalar!("SELECT MAX(generation) FROM (SELECT generation FROM analytics_fact_deltas WHERE owner_id=$1 AND generation>$2 ORDER BY generation LIMIT $3) batch", owner.as_str(), state.generation, limit).fetch_one(&mut *tx).await?;
        let Some(through) = through else {
            tx.commit().await?;
            return Ok(None);
        };
        let seconds = f64::from(lease_seconds);
        let updated = sqlx::query!(r#"UPDATE analytics_fact_consumers SET lease_worker=$3,lease_epoch=lease_epoch+1,lease_until=clock_timestamp()+make_interval(secs=>$4),lease_through=$5 WHERE owner_id=$1 AND consumer=$2 RETURNING lease_epoch,lease_until AS "lease_until!""#, owner.as_str(), consumer, worker.as_str(), seconds, through).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(DeltaLease {
            consumer: consumer.to_owned(),
            worker_id: worker.clone(),
            epoch: updated.lease_epoch,
            after_generation: state.generation,
            through_generation: through,
            expires_at: updated.lease_until,
        }))
    }

    pub async fn delta_batch(&self, owner: &UserId, lease: &DeltaLease) -> Result<Vec<FactDelta>> {
        let mut tx = self.pool.begin().await?;
        Self::lock_delta_lease(&mut tx, owner, lease).await?;
        let rows = sqlx::query!("SELECT generation,fact_kind,source,fact_id,before_fact,after_fact FROM analytics_fact_deltas WHERE owner_id=$1 AND generation>$2 AND generation<=$3 ORDER BY generation LIMIT 256", owner.as_str(), lease.after_generation, lease.through_generation).fetch_all(&mut *tx).await?;
        let deltas = rows
            .into_iter()
            .map(|row| {
                Ok(FactDelta {
                    generation: row.generation,
                    key: systemprompt_models::feedback::analytics::AnalyticsFactKey {
                        kind: validation::parse_kind(&row.fact_kind)?,
                        source: row.source,
                        id: systemprompt_identifiers::AnalyticsFactId::new(row.fact_id),
                    },
                    before: row.before_fact.map(serde_json::from_value).transpose()?,
                    after: row.after_fact.map(serde_json::from_value).transpose()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit().await?;
        Ok(deltas)
    }

    pub async fn lock_delta_lease(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        lease: &DeltaLease,
    ) -> Result<()> {
        let valid = sqlx::query_scalar!("SELECT generation FROM analytics_fact_consumers WHERE owner_id=$1 AND consumer=$2 AND lease_worker=$3 AND lease_epoch=$4 AND generation=$5 AND lease_through=$6 AND lease_until>clock_timestamp() FOR UPDATE", owner.as_str(), &lease.consumer, lease.worker_id.as_str(), lease.epoch, lease.after_generation, lease.through_generation).fetch_optional(&mut **tx).await?;
        if valid.is_none() {
            return Err(validation::invalid());
        }
        Ok(())
    }

    pub async fn complete_delta_batch(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        lease: &DeltaLease,
    ) -> Result<()> {
        Self::lock_delta_lease(tx, owner, lease).await?;
        let updated = sqlx::query!("UPDATE analytics_fact_consumers SET generation=$6,lease_worker=NULL,lease_until=NULL,lease_through=NULL WHERE owner_id=$1 AND consumer=$2 AND lease_worker=$3 AND lease_epoch=$4 AND generation=$5 AND lease_through=$6 AND lease_until>clock_timestamp()", owner.as_str(), &lease.consumer, lease.worker_id.as_str(), lease.epoch, lease.after_generation, lease.through_generation).execute(&mut **tx).await?;
        if updated.rows_affected() != 1 {
            return Err(validation::invalid());
        }
        sqlx::query!("UPDATE analytics_fact_deltas SET consumed_at=clock_timestamp() WHERE owner_id=$1 AND consumed_at IS NULL AND generation<=(SELECT MIN(generation) FROM analytics_fact_consumers WHERE owner_id=$1)", owner.as_str()).execute(&mut **tx).await?;
        Ok(())
    }
}
