//! [`BrainLocks`]: the brain's transaction-scoped advisory locks, so one
//! replica runs a cluster-wide pass at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_traits::RepositoryError;

const SWEEP_LOCK_KEY: i64 = 0x6272_6169_6e5f_7377;

/// The brain's advisory locks.
#[derive(Debug, Clone, Copy)]
pub struct BrainLocks;

impl BrainLocks {
    pub async fn try_sweep(conn: &mut PgConnection) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_xact_lock($1) AS "locked!""#,
            SWEEP_LOCK_KEY
        )
        .fetch_one(conn)
        .await?)
    }
}
