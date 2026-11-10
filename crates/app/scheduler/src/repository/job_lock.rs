//! Cross-replica job claim via Postgres session-scoped advisory locks.
//!
//! The in-process `RunningJobs` guard only prevents a single
//! process from running a job twice. When the scheduler runs as multiple
//! replicas against one database, every replica's cron fires the same job
//! on the same tick. [`JobLockRepository::try_acquire`] gives exactly one
//! replica the right to run a given job: the others observe a held lock and
//! skip. [`JobLockRepository::acquire_within`] is the waiting form a manual
//! run takes, polling the same lock until its deadline.
//!
//! Advisory locks are *session-scoped* — the connection that called
//! `pg_advisory_lock` is the only one that can release it. [`JobLockGuard`]
//! therefore pins the connection it locked on for the job's whole lifetime
//! and releases on that same connection, taken from the session pool so a
//! transaction-mode pooler in front of the main pool cannot hand the locked
//! backend to another client.
//!
//! Keys use the two-key form `(JOB_LOCK_CLASS, hashtext(job_name))`: the
//! class keeps job locks disjoint from every other advisory-lock user, and
//! the session holding a key is identified by both halves in `pg_locks`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use systemprompt_database::DbPool;
use systemprompt_identifiers::JobName;
use systemprompt_traits::RepositoryError;
use tracing::warn;

use crate::error::{SchedulerError, SchedulerResult};

// Why: Postgres advisory locks share one key space per database; a class of
// its own keeps job locks from colliding with instance or extension locks.
pub const JOB_LOCK_CLASS: i32 = 0x5350_0003;

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(250);

// Why: SQLx pool return keeps the session open; Postgres session locks
// survive until explicit unlock or session closure.
pub(crate) struct JobLockGuard {
    conn: Option<PoolConnection<Postgres>>,
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
            && let Err(e) = sqlx::query_scalar!(
                "SELECT pg_advisory_unlock($1, hashtext($2))",
                JOB_LOCK_CLASS,
                self.job_name.as_str()
            )
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
            .field("job_name", &self.job_name)
            .field("held", &self.conn.is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct JobLockRepository {
    session_pool: Arc<PgPool>,
}

impl JobLockRepository {
    pub(crate) fn new(db: &DbPool) -> Self {
        Self {
            session_pool: db.session_pool(),
        }
    }

    pub(crate) async fn try_acquire(
        &self,
        job_name: &JobName,
    ) -> SchedulerResult<Option<JobLockGuard>> {
        let mut conn = self.session_pool.acquire().await.map_err(lock_error)?;

        let acquired = sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_lock($1, hashtext($2)) AS "acquired!""#,
            JOB_LOCK_CLASS,
            job_name.as_str()
        )
        .fetch_one(conn.as_mut())
        .await
        .map_err(lock_error)?;

        if acquired {
            Ok(Some(JobLockGuard {
                conn: Some(conn),
                job_name: job_name.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn lock_keys(
        &self,
        names: &[JobName],
    ) -> SchedulerResult<Vec<(JobName, i32)>> {
        let raw: Vec<String> = names.iter().map(|n| n.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT name AS "name!", hashtext(name) AS "key!" FROM unnest($1::text[]) AS name"#,
            &raw
        )
        .fetch_all(&*self.session_pool)
        .await
        .map_err(lock_error)?;
        Ok(rows
            .into_iter()
            .map(|row| (JobName::new(row.name), row.key))
            .collect())
    }

    pub(crate) async fn acquire_within(
        &self,
        job_name: &JobName,
        wait: Duration,
    ) -> SchedulerResult<Option<JobLockGuard>> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(guard) = self.try_acquire(job_name).await? {
                return Ok(Some(guard));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            tokio::time::sleep(LOCK_POLL_INTERVAL.min(deadline - now)).await;
        }
    }
}

fn lock_error(err: sqlx::Error) -> SchedulerError {
    SchedulerError::DistributedLock(RepositoryError::from(err))
}
