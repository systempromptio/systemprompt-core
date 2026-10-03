//! Cross-replica job claim via Postgres session-scoped advisory locks.
//!
//! The in-process `RunningJobs` guard only prevents a single
//! process from running a job twice. When the scheduler runs as multiple
//! replicas against one database, every replica's cron fires the same job
//! on the same tick. [`JobLockRepository::try_acquire`] gives exactly one
//! replica the right to run a given job: the others observe a held lock and
//! skip.
//!
//! Advisory locks are *session-scoped* — the connection that called
//! `pg_advisory_lock` is the only one that can release it. [`JobLockGuard`]
//! therefore pins the connection it locked on for the job's whole lifetime
//! and releases on that same connection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use systemprompt_database::DbPool;
use systemprompt_identifiers::JobName;
use systemprompt_traits::RepositoryError;
use tracing::warn;

use crate::error::{SchedulerError, SchedulerResult};

// Why: SQLx pool return keeps the session open; Postgres session locks
// survive until explicit unlock or session closure.
pub(crate) struct JobLockGuard {
    conn: Option<PoolConnection<Postgres>>,
    key: i64,
    job_name: JobName,
}

impl Drop for JobLockGuard {
    fn drop(&mut self) {
        // Why: a connection returned to the pool keeps its session, and with it
        // the advisory lock; detaching closes the session so a cancelled job
        // cannot hold its claim until the pool recycles the connection.
        if let Some(conn) = self.conn.take() {
            warn!(
                job_name = %self.job_name,
                "JobLockGuard dropped without explicit release; closing its session"
            );
            let session = conn.detach();
            drop(session);
        }
    }
}

impl JobLockGuard {
    pub(crate) async fn release(mut self) {
        if let Some(mut conn) = self.conn.take()
            && let Err(e) = sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", self.key)
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

impl std::fmt::Debug for JobLockGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobLockGuard")
            .field("key", &self.key)
            .field("job_name", &self.job_name)
            .field("held", &self.conn.is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct JobLockRepository {
    write_pool: Arc<PgPool>,
}

impl JobLockRepository {
    pub(crate) fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    pub(crate) async fn try_acquire(
        &self,
        job_name: &JobName,
    ) -> SchedulerResult<Option<JobLockGuard>> {
        let mut conn = self.write_pool.acquire().await.map_err(lock_error)?;

        let key = sqlx::query_scalar!(
            r#"SELECT hashtext($1)::bigint AS "key!""#,
            job_name.as_str()
        )
        .fetch_one(conn.as_mut())
        .await
        .map_err(lock_error)?;

        let acquired =
            sqlx::query_scalar!(r#"SELECT pg_try_advisory_lock($1) AS "acquired!""#, key)
                .fetch_one(conn.as_mut())
                .await
                .map_err(lock_error)?;

        if acquired {
            Ok(Some(JobLockGuard {
                conn: Some(conn),
                key,
                job_name: job_name.clone(),
            }))
        } else {
            Ok(None)
        }
    }
}

fn lock_error(err: sqlx::Error) -> SchedulerError {
    SchedulerError::DistributedLock(RepositoryError::from(err))
}
