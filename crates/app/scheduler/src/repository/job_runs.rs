//! `scheduled_job_runs`: the claim of one run per job and scheduled instant.
//!
//! [`JobRunRepository::claim`] inserts the run row in the statement that
//! judges the claim (`ON CONFLICT DO NOTHING`), so there is no window between
//! reading whether an instant ran and claiming it: of any number of replicas
//! claiming one `(job, scheduled_at, instance_key)`, exactly one gets a
//! [`RunClaim`]. The key is the cron instant the tick belongs to, never the
//! wall-clock time a replica observed, so clock skew between replicas cannot
//! split one instant into two runs. A cluster-scoped job claims under the empty
//! instance key ([`RunKey::Cluster`]); a node-scoped job under its instance
//! ([`RunKey::Node`]).
//!
//! Every claim draws a [`FencingToken`] from
//! `scheduled_job_run_fencing_seq`; [`JobRunRepository::finish`] records the
//! outcome only on the row carrying that token, once.
//!
//! [`JobRunRepository::purge_settled_before`] is the bounded retention the
//! `database_cleanup` job's `scheduled_job_runs` sweep runs: rows older than
//! the cutoff, except each key's latest instant, which the missed-run policy
//! reads at start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{InstanceId, JobName};
use systemprompt_traits::FencingToken;

use crate::error::SchedulerResult;
use crate::models::JobStatus;

/// Which replicas share a run row for one instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunKey {
    Cluster,
    Node(InstanceId),
}

impl RunKey {
    #[must_use]
    pub fn as_key(&self) -> &str {
        match self {
            Self::Cluster => "",
            Self::Node(instance) => instance.as_str(),
        }
    }
}

/// The won claim of one job's run for one scheduled instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunClaim {
    job_name: JobName,
    scheduled_at: DateTime<Utc>,
    key: RunKey,
    token: FencingToken,
}

impl RunClaim {
    #[must_use]
    pub const fn job_name(&self) -> &JobName {
        &self.job_name
    }

    #[must_use]
    pub const fn scheduled_at(&self) -> DateTime<Utc> {
        self.scheduled_at
    }

    #[must_use]
    pub const fn key(&self) -> &RunKey {
        &self.key
    }

    #[must_use]
    pub const fn token(&self) -> FencingToken {
        self.token
    }
}

#[derive(Debug, Clone)]
pub struct JobRunRepository {
    write_pool: Arc<PgPool>,
}

impl JobRunRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    pub async fn claim(
        &self,
        job_name: &JobName,
        scheduled_at: DateTime<Utc>,
        key: &RunKey,
        claimed_by: &InstanceId,
    ) -> SchedulerResult<Option<RunClaim>> {
        let token = sqlx::query_scalar!(
            r#"
            INSERT INTO scheduled_job_runs
                (job_name, scheduled_at, instance_key, claimed_by, fencing_token)
            VALUES ($1, $2, $3, $4, nextval('scheduled_job_run_fencing_seq'))
            ON CONFLICT (job_name, scheduled_at, instance_key) DO NOTHING
            RETURNING fencing_token
            "#,
            job_name.as_str(),
            scheduled_at,
            key.as_key(),
            claimed_by.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        Ok(token.map(|token| RunClaim {
            job_name: job_name.clone(),
            scheduled_at,
            key: key.clone(),
            token: FencingToken::new(token),
        }))
    }

    pub async fn finish(&self, claim: &RunClaim, status: JobStatus) -> SchedulerResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE scheduled_job_runs
            SET finished_at = CURRENT_TIMESTAMP, status = $5
            WHERE job_name = $1 AND scheduled_at = $2 AND instance_key = $3
              AND fencing_token = $4 AND finished_at IS NULL
            "#,
            claim.job_name.as_str(),
            claim.scheduled_at,
            claim.key.as_key(),
            claim.token.get(),
            status.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn last_scheduled_at(
        &self,
        job_name: &JobName,
        key: &RunKey,
    ) -> SchedulerResult<Option<DateTime<Utc>>> {
        Ok(sqlx::query_scalar!(
            r#"
            SELECT max(scheduled_at) AS "last"
            FROM scheduled_job_runs
            WHERE job_name = $1 AND instance_key = $2
            "#,
            job_name.as_str(),
            key.as_key()
        )
        .fetch_one(&*self.write_pool)
        .await?)
    }

    pub async fn count_runs(&self, job_name: &JobName) -> SchedulerResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM scheduled_job_runs WHERE job_name = $1"#,
            job_name.as_str()
        )
        .fetch_one(&*self.write_pool)
        .await?)
    }

    pub async fn purge_settled_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> SchedulerResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM scheduled_job_runs
            WHERE ctid IN (
                SELECT r.ctid FROM scheduled_job_runs r
                WHERE r.scheduled_at < $1
                  AND r.scheduled_at < (
                      SELECT max(l.scheduled_at) FROM scheduled_job_runs l
                      WHERE l.job_name = r.job_name AND l.instance_key = r.instance_key
                  )
                LIMIT $2
            )
            "#,
            cutoff,
            batch
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
