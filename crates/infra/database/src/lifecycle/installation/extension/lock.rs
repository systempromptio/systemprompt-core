//! Session-pinned Postgres advisory lock serialising concurrent bootstraps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use systemprompt_extension::LoaderError;
use tracing::{debug, warn};

use crate::services::DatabaseProvider;

/// Every `systemprompt` process must lock on this same value for the advisory
/// lock to serialise concurrent boots.
const BOOTSTRAP_ADVISORY_LOCK_KEY: i64 = 0x73_70_72_6F_6D_70_74_01;

/// Holds the dedicated Postgres session that owns the bootstrap advisory lock.
///
/// `pg_advisory_lock` is session-scoped: only the backend that acquired the
/// lock can release it. The guard pins one [`PoolConnection`] for the install's
/// lifetime so acquire and release run on the same session. Non-Postgres
/// providers skip locking — bootstrap concurrency is a Postgres-only concern.
pub(super) struct BootstrapLockGuard {
    conn: Option<PoolConnection<Postgres>>,
}

impl BootstrapLockGuard {
    pub(super) async fn acquire(db: &dyn DatabaseProvider) -> Result<Self, LoaderError> {
        let Some(pool) = db.get_postgres_pool() else {
            return Ok(Self { conn: None });
        };

        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| LoaderError::SchemaInstallationFailed {
                extension: "database".to_owned(),
                message: format!("Failed to acquire bootstrap lock connection: {e}"),
            })?;

        sqlx::query!("SELECT pg_advisory_lock($1)", BOOTSTRAP_ADVISORY_LOCK_KEY)
            .execute(conn.as_mut())
            .await
            .map_err(|e| LoaderError::SchemaInstallationFailed {
                extension: "database".to_owned(),
                message: format!("Failed to acquire bootstrap advisory lock: {e}"),
            })?;

        debug!(
            key = BOOTSTRAP_ADVISORY_LOCK_KEY,
            "Acquired bootstrap advisory lock"
        );

        Ok(Self { conn: Some(conn) })
    }

    pub(super) async fn release(mut self) {
        if let Some(mut conn) = self.conn.take()
            && let Err(e) =
                sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", BOOTSTRAP_ADVISORY_LOCK_KEY)
                    .fetch_one(conn.as_mut())
                    .await
        {
            warn!(
                error = %e,
                "Failed to release bootstrap advisory lock; connection recycle will clear it"
            );
        }
    }
}

impl Drop for BootstrapLockGuard {
    fn drop(&mut self) {
        if self.conn.is_some() {
            warn!(
                key = BOOTSTRAP_ADVISORY_LOCK_KEY,
                "BootstrapLockGuard dropped without explicit release; lock will clear when the \
                 pooled connection recycles"
            );
        }
    }
}
