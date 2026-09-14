//! Idempotent asynchronous range requests with restart-safe worker fencing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ranges::Range;
use super::{
    FeedbackSnapshotsRepository, SnapshotJobLease, SnapshotRangeJob, SnapshotRangeRequest, invalid,
};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{TaskId, UserId};

impl FeedbackSnapshotsRepository {
    pub async fn request_range(
        &self,
        owner: &UserId,
        request: &SnapshotRangeRequest,
        now: DateTime<Utc>,
    ) -> crate::Result<SnapshotRangeJob> {
        if request.operation_id.as_str().is_empty()
            || request.operation_id.as_str().len() > 180
            || request
                .resource_id
                .as_ref()
                .is_some_and(|id| id.as_str().is_empty() || id.as_str().len() > 180)
        {
            return Err(invalid("Invalid range operation identity"));
        }
        let days = (request.to_day - request.from_day).num_days();
        if !(1..=365).contains(&days)
            || request.from_day < now.date_naive() - chrono::Duration::days(364)
            || request.to_day > now.date_naive() + chrono::Duration::days(1)
        {
            return Err(invalid(
                "Range must contain 1–365 UTC days within retention",
            ));
        }
        let scope = request.resource_id.as_ref().map_or("", |id| id.as_str());
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO analytics_snapshot_state(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "SELECT generation FROM analytics_snapshot_state WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        let existing=sqlx::query!("SELECT scope,from_day,to_day FROM analytics_snapshot_jobs WHERE owner_id=$1 AND job_id=$2",owner.as_str(),request.operation_id.as_str()).fetch_optional(&mut *tx).await?;
        if let Some(row) = existing {
            if row.scope != scope
                || row.from_day != request.from_day
                || row.to_day != request.to_day
            {
                return Err(invalid("Conflicting range operation retry"));
            }
        } else {
            let count=sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM analytics_snapshot_jobs WHERE owner_id=$1 AND state IN('pending','leased')"#,owner.as_str()).fetch_one(&mut *tx).await?;
            if count >= 100 {
                return Err(invalid("Pending range job limit reached"));
            }
            sqlx::query!("INSERT INTO analytics_snapshot_jobs(owner_id,job_id,scope,from_day,to_day) VALUES($1,$2,$3,$4,$5)",owner.as_str(),request.operation_id.as_str(),scope,request.from_day,request.to_day).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.range_job(owner, &request.operation_id)
            .await?
            .ok_or_else(|| invalid("Range operation unavailable"))
    }
    pub async fn range_job(
        &self,
        owner: &UserId,
        job: &TaskId,
    ) -> crate::Result<Option<SnapshotRangeJob>> {
        let row=sqlx::query!("SELECT state,result,last_error FROM analytics_snapshot_jobs WHERE owner_id=$1 AND job_id=$2",owner.as_str(),job.as_str()).fetch_optional(&self.pool).await?;
        row.map(|row| {
            Ok(SnapshotRangeJob {
                operation_id: job.clone(),
                state: row.state,
                result: row.result.map(serde_json::from_value).transpose()?,
                diagnostic: row.last_error,
            })
        })
        .transpose()
    }
    pub async fn claim_range(
        &self,
        owner: &UserId,
        worker: &TaskId,
    ) -> crate::Result<Option<SnapshotJobLease>> {
        let mut tx = self.pool.begin().await?;
        let row=sqlx::query!("SELECT job_id FROM analytics_snapshot_jobs WHERE owner_id=$1 AND (state='pending' OR (state='leased' AND lease_until<=clock_timestamp())) ORDER BY created_at,job_id FOR UPDATE SKIP LOCKED LIMIT 1",owner.as_str()).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let epoch=sqlx::query_scalar!("UPDATE analytics_snapshot_jobs SET state='leased',lease_worker=$3,lease_epoch=lease_epoch+1,lease_until=clock_timestamp()+interval '5 minutes' WHERE owner_id=$1 AND job_id=$2 RETURNING lease_epoch",owner.as_str(),&row.job_id,worker.as_str()).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(SnapshotJobLease {
            operation_id: TaskId::new(row.job_id),
            worker_id: worker.clone(),
            epoch,
        }))
    }
    pub async fn complete_range(
        &self,
        owner: &UserId,
        lease: &SnapshotJobLease,
        now: DateTime<Utc>,
    ) -> crate::Result<()> {
        let mut tx = self.pool.begin().await?;
        let state=sqlx::query!("SELECT generation,fact_generation FROM analytics_snapshot_state WHERE owner_id=$1 FOR UPDATE",owner.as_str()).fetch_one(&mut *tx).await?;
        let row=sqlx::query!("SELECT scope,from_day,to_day FROM analytics_snapshot_jobs WHERE owner_id=$1 AND job_id=$2 AND state='leased' AND lease_worker=$3 AND lease_epoch=$4 AND lease_until>clock_timestamp() FOR UPDATE",owner.as_str(),lease.operation_id.as_str(),lease.worker_id.as_str(),lease.epoch).fetch_optional(&mut *tx).await?.ok_or_else(||invalid("Stale range job lease"))?;
        let snapshot = Self::assemble(
            &mut tx,
            owner,
            &Range {
                scope: &row.scope,
                from: row.from_day,
                to: row.to_day,
                generation: state.generation,
                fact_generation: state.fact_generation,
                now,
            },
        )
        .await?;
        let result = serde_json::to_value(snapshot)?;
        let changed=sqlx::query!("UPDATE analytics_snapshot_jobs SET state='ready',result=$5,completed_at=clock_timestamp(),lease_worker=NULL,lease_until=NULL WHERE owner_id=$1 AND job_id=$2 AND lease_worker=$3 AND lease_epoch=$4 AND lease_until>clock_timestamp()",owner.as_str(),lease.operation_id.as_str(),lease.worker_id.as_str(),lease.epoch,result).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(invalid("Expired range job lease"));
        }
        tx.commit().await?;
        Ok(())
    }
}
