//! [`AdvisoryLocks`] for extensions: session-scoped Postgres advisory locks
//! taken on the session pool.
//!
//! An advisory lock belongs to the backend session that took it, so the
//! connection must stay pinned until release and must never come from a pool
//! behind a transaction-mode pooler, which would hand the locked backend to
//! another client between statements. Keys use the two-key form under
//! [`EXTENSION_LOCK_CLASS`], disjoint from the scheduler's single-key job
//! locks and from the instance claims.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use systemprompt_identifiers::AdvisoryLockKey;
use systemprompt_traits::{
    AdvisoryLockGuard, AdvisoryLocks, CoordinationResult, HeldAdvisoryLock, RepositoryError,
};
use tokio::time::Instant;
use tracing::warn;

use crate::DbPool;

// Why: Postgres advisory locks share one key space per database; a class of
// its own keeps extension locks from colliding with job or instance locks.
pub const EXTENSION_LOCK_CLASS: i32 = 0x5350_0002;

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct SessionAdvisoryLocks {
    session_pool: Arc<PgPool>,
}

impl SessionAdvisoryLocks {
    pub fn new(db: &DbPool) -> Self {
        Self {
            session_pool: db.session_pool(),
        }
    }
}

#[async_trait]
impl AdvisoryLocks for SessionAdvisoryLocks {
    async fn try_acquire(
        &self,
        key: &AdvisoryLockKey,
    ) -> CoordinationResult<Option<AdvisoryLockGuard>> {
        let mut conn = self
            .session_pool
            .acquire()
            .await
            .map_err(RepositoryError::from)?;
        let acquired = sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_lock($1, hashtext($2)) AS "acquired!""#,
            EXTENSION_LOCK_CLASS,
            key.as_str()
        )
        .fetch_one(conn.as_mut())
        .await
        .map_err(RepositoryError::from)?;
        if !acquired {
            return Ok(None);
        }
        Ok(Some(Box::new(SessionAdvisoryLock {
            conn: Some(conn),
            key: key.clone(),
        })))
    }

    async fn acquire_within(
        &self,
        key: &AdvisoryLockKey,
        wait: Duration,
    ) -> CoordinationResult<Option<AdvisoryLockGuard>> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(guard) = self.try_acquire(key).await? {
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

pub struct SessionAdvisoryLock {
    conn: Option<PoolConnection<Postgres>>,
    key: AdvisoryLockKey,
}

impl std::fmt::Debug for SessionAdvisoryLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAdvisoryLock")
            .field("key", &self.key)
            .field("held", &self.conn.is_some())
            .finish()
    }
}

impl Drop for SessionAdvisoryLock {
    fn drop(&mut self) {
        // Why: a connection returned to the pool keeps its session, and with it
        // the lock; detaching closes the session so the lock is freed now.
        if let Some(conn) = self.conn.take() {
            warn!(
                key = %self.key,
                "advisory lock dropped without explicit release; closing its session"
            );
            let session = conn.detach();
            drop(session);
        }
    }
}

#[async_trait]
impl HeldAdvisoryLock for SessionAdvisoryLock {
    fn key(&self) -> &AdvisoryLockKey {
        &self.key
    }

    async fn release(mut self: Box<Self>) {
        let Some(mut conn) = self.conn.take() else {
            return;
        };
        let unlocked = sqlx::query_scalar!(
            r#"SELECT pg_advisory_unlock($1, hashtext($2)) AS "unlocked!""#,
            EXTENSION_LOCK_CLASS,
            self.key.as_str()
        )
        .fetch_one(conn.as_mut())
        .await;
        match unlocked {
            Ok(true) => {},
            Ok(false) => {
                warn!(key = %self.key, "advisory lock was not held at release; closing its session");
                let session = conn.detach();
                drop(session);
            },
            Err(error) => {
                warn!(key = %self.key, %error, "advisory lock release failed; closing its session");
                let session = conn.detach();
                drop(session);
            },
        }
    }
}
