//! Cross-replica job claim via Postgres session-scoped advisory locks.
//!
//! The in-process [`super::RunningJobs`] guard only prevents a single
//! process from running a job twice. When the scheduler runs as multiple
//! replicas against one database, every replica's cron fires the same job
//! on the same tick. [`try_acquire_job_lock`] gives exactly one replica the
//! right to run a given job: the others observe a held lock and skip.
//!
//! Advisory locks are *session-scoped* — the connection that called
//! `pg_advisory_lock` is the only one that can release it. [`JobLockGuard`]
//! therefore pins the connection it locked on for the job's whole lifetime
//! and releases on that same connection.

use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use tracing::warn;

use crate::error::{SchedulerError, SchedulerResult};

/// Holds the dedicated connection an advisory lock was taken on.
///
/// Call [`JobLockGuard::release`] once the job body finishes. [`Drop`] is a
/// safety net only: if `release` was missed, the connection is dropped and
/// returned to the pool, and Postgres releases all session advisory locks
/// when that pooled connection is eventually recycled or closed.
pub(super) struct JobLockGuard {
    conn: Option<PoolConnection<Postgres>>,
    key: i64,
    job_name: String,
}

impl JobLockGuard {
    pub(super) async fn release(mut self) {
        if let Some(mut conn) = self.conn.take() {
            if let Err(e) = sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", self.key)
                .fetch_one(conn.as_mut())
                .await
            {
                warn!(
                    job_name = %self.job_name,
                    error = %e,
                    "Failed to release job advisory lock; connection recycle will clear it"
                );
            }
        }
    }
}

impl std::fmt::Debug for JobLockGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobLockGuard")
            .field("key", &self.key)
            .field("job_name", &self.job_name)
            .field("held", &self.conn.is_some())
            .finish()
    }
}

pub(super) async fn try_acquire_job_lock(
    write_pool: &PgPool,
    job_name: &str,
) -> SchedulerResult<Option<JobLockGuard>> {
    let mut conn = write_pool
        .acquire()
        .await
        .map_err(|e| SchedulerError::DistributedLock(e.to_string()))?;

    let key = sqlx::query_scalar!(r#"SELECT hashtext($1)::bigint AS "key!""#, job_name)
        .fetch_one(conn.as_mut())
        .await
        .map_err(|e| SchedulerError::DistributedLock(e.to_string()))?;

    let acquired = sqlx::query_scalar!(r#"SELECT pg_try_advisory_lock($1) AS "acquired!""#, key)
        .fetch_one(conn.as_mut())
        .await
        .map_err(|e| SchedulerError::DistributedLock(e.to_string()))?;

    if acquired {
        Ok(Some(JobLockGuard {
            conn: Some(conn),
            key,
            job_name: job_name.to_owned(),
        }))
    } else {
        Ok(None)
    }
}
