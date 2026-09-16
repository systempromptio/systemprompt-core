//! Session-pinned Postgres advisory lock serialising concurrent bootstraps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use systemprompt_extension::LoaderError;
use tracing::{debug, warn};

use crate::services::DatabaseProvider;

pub const BOOTSTRAP_ADVISORY_LOCK_KEY: i64 = 0x73_70_72_6F_6D_70_74_01;

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

impl BootstrapLockGuard {
    pub async fn acquire(db: &dyn DatabaseProvider) -> Result<Self, LoaderError> {
        let mut conn = db.get_postgres_pool().acquire().await.map_err(|e| {
            LoaderError::SchemaInstallationFailed {
                extension: "database".to_owned(),
                message: format!("Failed to acquire bootstrap lock connection: {e}"),
            }
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
