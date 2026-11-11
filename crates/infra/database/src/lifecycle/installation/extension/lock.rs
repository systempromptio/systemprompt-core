//! Session-pinned Postgres advisory lock serialising concurrent bootstraps.
//!
//! Acquisition polls `pg_try_advisory_lock` with a deadline and logs progress,
//! so a wedged holder surfaces as a typed [`BootstrapLockTimeout`] rather than
//! an unbounded silent wait.
//!
//! While a server boot waits here the early-bind listener is already serving:
//! `/livez` answers `200` and `/readyz` answers `503 starting`, so a liveness
//! probe keeps the replica alive and no traffic is admitted. When the wait
//! runs out the boot fails with that typed error and the process exits for
//! the orchestrator to restart; a startup probe with a budget shorter than
//! [`BootstrapLockGuard::DEFAULT_WAIT`] restarts it sooner instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::services::DatabaseProvider;

pub const BOOTSTRAP_ADVISORY_LOCK_KEY: i64 = 0x73_70_72_6F_6D_70_74_01;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const PROGRESS_INTERVAL: Duration = Duration::from_secs(30);

/// Session-pinned advisory lock serialising concurrent bootstraps.
///
/// Only the acquiring Postgres session can release its advisory lock, so the
/// guard pins that connection. Dropping the guard without `release` closes
/// the session instead of returning it to the pool, so the lock never
/// outlives a cancelled or panicking holder.
pub struct BootstrapLockGuard {
    conn: Option<PoolConnection<Postgres>>,
}

impl std::fmt::Debug for BootstrapLockGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootstrapLockGuard")
            .field("key", &BOOTSTRAP_ADVISORY_LOCK_KEY)
            .field("held", &self.conn.is_some())
            .finish()
    }
}

/// The bootstrap lock was still held by another session when the wait ran out.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("bootstrap advisory lock still held by another session after {waited:?}")]
pub struct BootstrapLockTimeout {
    pub waited: Duration,
}

impl BootstrapLockGuard {
    pub const DEFAULT_WAIT: Duration = Duration::from_mins(15);

    pub async fn acquire(db: &dyn DatabaseProvider) -> Result<Self, LoaderError> {
        Self::acquire_within(db, Self::DEFAULT_WAIT).await
    }

    pub async fn acquire_within(
        db: &dyn DatabaseProvider,
        wait: Duration,
    ) -> Result<Self, LoaderError> {
        let mut conn = db.get_postgres_pool().acquire().await.map_err(|e| {
            LoaderError::SchemaInstallationStepFailed {
                extension: ExtensionId::new("database"),
                context: "Failed to acquire bootstrap lock connection".to_owned(),
                source: Box::new(e),
            }
        })?;

        let started = Instant::now();
        let deadline = started + wait;
        let mut next_progress = started + PROGRESS_INTERVAL;
        loop {
            let acquired = sqlx::query_scalar!(
                r#"SELECT pg_try_advisory_lock($1) AS "acquired!""#,
                BOOTSTRAP_ADVISORY_LOCK_KEY
            )
            .fetch_one(conn.as_mut())
            .await
            .map_err(|e| LoaderError::SchemaInstallationStepFailed {
                extension: ExtensionId::new("database"),
                context: "Failed to acquire bootstrap advisory lock".to_owned(),
                source: Box::new(e),
            })?;
            if acquired {
                break;
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(LoaderError::SchemaInstallationStepFailed {
                    extension: ExtensionId::new("database"),
                    context: "Timed out waiting for the bootstrap advisory lock".to_owned(),
                    source: Box::new(BootstrapLockTimeout { waited: wait }),
                });
            }
            if now >= next_progress {
                info!(
                    key = BOOTSTRAP_ADVISORY_LOCK_KEY,
                    waited_secs = now.duration_since(started).as_secs(),
                    "Waiting for another session to release the bootstrap advisory lock"
                );
                next_progress = now + PROGRESS_INTERVAL;
            }
            tokio::time::sleep(POLL_INTERVAL.min(deadline - now)).await;
        }

        debug!(
            key = BOOTSTRAP_ADVISORY_LOCK_KEY,
            "Acquired bootstrap advisory lock"
        );

        Ok(Self { conn: Some(conn) })
    }

    pub async fn release(mut self) {
        let Some(mut conn) = self.conn.take() else {
            return;
        };
        match sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", BOOTSTRAP_ADVISORY_LOCK_KEY)
            .fetch_one(conn.as_mut())
            .await
        {
            Ok(Some(true)) => drop(conn),
            Ok(released) => {
                warn!(
                    key = BOOTSTRAP_ADVISORY_LOCK_KEY,
                    ?released,
                    "Bootstrap advisory lock was not held by this session at release"
                );
                drop(conn);
            },
            Err(e) => {
                warn!(
                    error = %e,
                    "Failed to release bootstrap advisory lock; closing its session instead of pooling it"
                );
                let session = conn.detach();
                drop(session);
            },
        }
    }
}

impl Drop for BootstrapLockGuard {
    fn drop(&mut self) {
        // Why: a connection returned to the pool keeps its session, and with it
        // the advisory lock; detaching closes the session so a cancelled or
        // panicking install cannot leave every other replica blocked.
        if let Some(conn) = self.conn.take() {
            warn!(
                key = BOOTSTRAP_ADVISORY_LOCK_KEY,
                "BootstrapLockGuard dropped without explicit release; closing its session"
            );
            let session = conn.detach();
            drop(session);
        }
    }
}
