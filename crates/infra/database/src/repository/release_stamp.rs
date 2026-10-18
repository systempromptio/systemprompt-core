//! [`ReleaseStampRepository`]: run a piece of work once per release across
//! every replica — content ingestion after a deploy, a one-off backfill.
//!
//! The run holds a session advisory lock keyed on the stamp name, so replicas
//! booting together serialise; it reads the release recorded under the name,
//! runs the work when this release is ahead (or, under
//! [`ReleaseGate::UnlessNewer`], not behind), and records this release only
//! when the work succeeds. A binary older than the recorded release never
//! runs the work, so a lagging pod in a rolling update cannot redo a newer
//! release's ingest with its stale content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_identifiers::{AdvisoryLockKey, JobName};
use systemprompt_traits::{AdvisoryLocks, CoordinationError, RepositoryError};

use super::advisory_lock::SessionAdvisoryLocks;
use crate::DbPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseGate {
    IfBehind,
    UnlessNewer,
}

#[derive(Debug, Clone, Copy)]
pub struct ReleaseRun<'a> {
    pub name: &'a JobName,
    pub release: &'a str,
    pub gate: ReleaseGate,
    pub wait: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampOutcome<T> {
    Ran(T),
    Current,
    NewerRecorded { recorded: String },
    Contended,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseStampError<E> {
    #[error("release stamp lock failed")]
    Lock(#[source] CoordinationError),
    #[error("release stamp could not be read or written")]
    Repository(#[source] RepositoryError),
    #[error("release-stamped run failed")]
    Run(#[source] E),
}

#[derive(Debug, Clone)]
pub struct ReleaseStampRepository {
    write_pool: Arc<PgPool>,
    locks: SessionAdvisoryLocks,
}

#[must_use]
pub fn release_key(version: &str) -> Vec<u64> {
    version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

impl ReleaseStampRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
            locks: SessionAdvisoryLocks::new(db),
        }
    }

    pub async fn recorded(&self, name: &JobName) -> Result<Option<String>, RepositoryError> {
        let version = sqlx::query_scalar!(
            "SELECT version FROM release_stamps WHERE name = $1",
            name.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        Ok(version)
    }

    async fn stamp(&self, name: &JobName, release: &str) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO release_stamps (name, version, stamped_at)
             VALUES ($1, $2, CURRENT_TIMESTAMP)
             ON CONFLICT (name) DO UPDATE SET version = EXCLUDED.version,
                                              stamped_at = EXCLUDED.stamped_at",
            name.as_str(),
            release
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn run_once<F, Fut, T, E>(
        &self,
        run: ReleaseRun<'_>,
        work: F,
    ) -> Result<StampOutcome<T>, ReleaseStampError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::error::Error + 'static,
    {
        let key = AdvisoryLockKey::new(format!("release_stamp:{}", run.name.as_str()));
        let Some(guard) = self
            .locks
            .acquire_within(&key, run.wait)
            .await
            .map_err(ReleaseStampError::Lock)?
        else {
            return Ok(StampOutcome::Contended);
        };
        let outcome = self.run_locked(run, work).await;
        guard.release().await;
        outcome
    }

    async fn run_locked<F, Fut, T, E>(
        &self,
        run: ReleaseRun<'_>,
        work: F,
    ) -> Result<StampOutcome<T>, ReleaseStampError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::error::Error + 'static,
    {
        let recorded = self
            .recorded(run.name)
            .await
            .map_err(ReleaseStampError::Repository)?;
        if let Some(recorded) = recorded {
            let ours = release_key(run.release);
            let theirs = release_key(&recorded);
            if theirs > ours {
                tracing::warn!(
                    name = %run.name,
                    recorded = %recorded,
                    release = run.release,
                    "release-stamped run skipped: a newer release already ran it"
                );
                return Ok(StampOutcome::NewerRecorded { recorded });
            }
            if run.gate == ReleaseGate::IfBehind && theirs == ours {
                return Ok(StampOutcome::Current);
            }
        }
        let value = work().await.map_err(ReleaseStampError::Run)?;
        self.stamp(run.name, run.release)
            .await
            .map_err(ReleaseStampError::Repository)?;
        Ok(StampOutcome::Ran(value))
    }
}
