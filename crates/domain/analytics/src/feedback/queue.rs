//! Leases fence every completion; committed pending rows remain discoverable
//! after restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FactLease, FactsHealth, FeedbackFactsRepository, validation};
use crate::Result;
use systemprompt_identifiers::{AnalyticsChangeId, AnalyticsWorkerId, UserId};

impl FeedbackFactsRepository {
    pub async fn claim(
        &self,
        owner: &UserId,
        worker: &AnalyticsWorkerId,
        limit: u32,
        lease_seconds: u32,
    ) -> Result<Vec<FactLease>> {
        if !(1..=256).contains(&limit) || !(1..=300).contains(&lease_seconds) {
            return Err(validation::invalid());
        }
        let limit = i64::from(limit);
        let seconds = f64::from(lease_seconds);
        let rows = sqlx::query!(r#"WITH ready AS (SELECT change_id FROM analytics_fact_changes WHERE owner_id=$1 AND next_attempt_at<=now() AND (state='pending' OR (state='leased' AND lease_until<=now())) ORDER BY recorded_at,change_id LIMIT $3 FOR UPDATE SKIP LOCKED) UPDATE analytics_fact_changes c SET state='leased',lease_worker=$2,lease_epoch=c.lease_epoch+1,lease_until=now()+make_interval(secs=>$4),attempts=c.attempts+1 FROM ready WHERE c.owner_id=$1 AND c.change_id=ready.change_id RETURNING c.change_id,c.lease_epoch,c.lease_until AS "lease_until!""#, owner.as_str(), worker.as_str(), limit, seconds).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| FactLease {
                change_id: AnalyticsChangeId::new(row.change_id),
                worker_id: worker.clone(),
                epoch: row.lease_epoch,
                expires_at: row.lease_until,
            })
            .collect())
    }

    pub async fn retry(&self, owner: &UserId, lease: &FactLease) -> Result<()> {
        let result = sqlx::query!("UPDATE analytics_fact_changes SET state='pending',lease_worker=NULL,lease_until=NULL,last_error='Fact processing failed; retry scheduled',next_attempt_at=now()+make_interval(secs=>LEAST(attempts,60)::double precision) WHERE owner_id=$1 AND change_id=$2 AND state='leased' AND lease_worker=$3 AND lease_epoch=$4 AND lease_until>now()", owner.as_str(), lease.change_id.as_str(), lease.worker_id.as_str(), lease.epoch).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            return Err(validation::invalid());
        }
        Ok(())
    }

    pub async fn health(&self, owner: &UserId) -> Result<FactsHealth> {
        let checkpoint = sqlx::query!("SELECT generation,last_applied_at,last_recorded_at FROM analytics_fact_checkpoints WHERE owner_id=$1", owner.as_str()).fetch_optional(&self.pool).await?;
        let queue = sqlx::query!(r#"SELECT COUNT(*) FILTER(WHERE state='pending') AS "pending!", COUNT(*) FILTER(WHERE state='leased') AS "leased!", COUNT(*) FILTER(WHERE last_error IS NOT NULL AND state IN ('pending','leased')) AS "retries!", MIN(recorded_at) FILTER(WHERE state IN ('pending','leased')) AS oldest_pending_at FROM analytics_fact_changes WHERE owner_id=$1"#, owner.as_str()).fetch_one(&self.pool).await?;
        Ok(FactsHealth {
            generation: checkpoint.as_ref().map_or(0, |row| row.generation),
            pending: queue.pending,
            leased: queue.leased,
            retries: queue.retries,
            oldest_pending_at: queue.oldest_pending_at,
            last_applied_at: checkpoint.as_ref().and_then(|row| row.last_applied_at),
            last_recorded_at: checkpoint.and_then(|row| row.last_recorded_at),
        })
    }
}
